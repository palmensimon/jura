use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::TextArea;

use crate::{
    config::{Config, JiraConfig, save_config, save_settings},
    tui::app::{App, AppEvent, AppView},
};

const F_BASE_URL: usize = 0;
const F_TOKEN: usize = 1;
const F_PROJECT: usize = 2;
const F_SPRINT: usize = 3;
const FIELD_COUNT: usize = 4;

pub struct SettingsState {
    inputs: [TextArea<'static>; FIELD_COUNT],
    active: usize,
    editing: bool,
}

impl SettingsState {
    pub fn new(config: &Config) -> Self {
        let mut inputs = std::array::from_fn(|_| TextArea::default());
        inputs[F_BASE_URL] = single_line_area(&config.jira.base_url);
        inputs[F_TOKEN] = single_line_area(&config.jira.token);
        inputs[F_PROJECT] = single_line_area(config.defaults.project.as_deref().unwrap_or(""));
        inputs[F_SPRINT] = single_line_area(&config.active_sprint_id.map(|id| id.to_string()).unwrap_or_default());

        let mut state = Self { inputs, active: 0, editing: false };
        state.refresh_styles();
        state
    }

    fn refresh_styles(&mut self) {
        for (i, input) in self.inputs.iter_mut().enumerate() {
            let focused = i == self.active;
            let editing = focused && self.editing;
            update_field_block(input, field_label(i), focused, editing);
        }
    }

    fn move_to(&mut self, idx: usize) {
        self.editing = false;
        self.active = idx;
        self.refresh_styles();
    }

    fn move_next(&mut self) {
        self.move_to((self.active + 1) % FIELD_COUNT);
    }

    fn move_prev(&mut self) {
        self.move_to((self.active + FIELD_COUNT - 1) % FIELD_COUNT);
    }

    fn first_line(&self, idx: usize) -> String {
        self.inputs[idx].lines().first().cloned().unwrap_or_default().trim().to_string()
    }

    fn build_config(&self, existing: &Config) -> Result<Config, String> {
        let base_url = self.first_line(F_BASE_URL);
        let token = self.first_line(F_TOKEN);
        let project = self.first_line(F_PROJECT);
        let sprint = self.first_line(F_SPRINT);
        if base_url.is_empty() {
            return Err("Base URL is required".to_string());
        }
        if token.is_empty() {
            return Err("Token / PAT is required".to_string());
        }
        let active_sprint_id = if sprint.is_empty() {
            None
        } else {
            Some(sprint.parse::<u64>().map_err(|_| "Active Sprint ID must be a number".to_string())?)
        };
        let mut defaults = existing.defaults.clone();
        defaults.project = if project.is_empty() { None } else { Some(project) };
        Ok(Config {
            jira: JiraConfig { base_url, token },
            active_sprint_id,
            defaults,
        })
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut SettingsState, key: KeyEvent) {
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        match state.build_config(&app.config) {
            Ok(new_cfg) => {
                let save_result = save_config(&new_cfg)
                    .and_then(|_| save_settings(&new_cfg.defaults));
                if let Err(e) = save_result {
                    app.error = Some(format!("Save failed: {e:#}"));
                } else {
                    let tx = app.event_tx.clone();
                    let cfg = new_cfg.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(AppEvent::ConfigSaved(cfg)).await;
                    });
                }
            }
            Err(msg) => app.error = Some(msg),
        }
        return;
    }

    if state.editing {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                state.editing = false;
                state.refresh_styles();
            }
            _ => { state.inputs[state.active].input(key); }
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.view = AppView::TicketList;
            app.error = None;
        }
        KeyCode::Char(' ') => {
            state.editing = true;
            state.refresh_styles();
        }
        KeyCode::Tab | KeyCode::Down => state.move_next(),
        KeyCode::BackTab | KeyCode::Up => state.move_prev(),
        KeyCode::Char('1') => state.move_to(F_BASE_URL),
        KeyCode::Char('2') => state.move_to(F_TOKEN),
        KeyCode::Char('3') => state.move_to(F_PROJECT),
        KeyCode::Char('4') => state.move_to(F_SPRINT),
        _ => {}
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────

pub fn draw(app: &App, state: &mut SettingsState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Length(3), // base_url
            Constraint::Length(3), // token
            Constraint::Length(3), // project
            Constraint::Length(3), // active_sprint_id
            Constraint::Min(0),    // user_settings.yaml info
            Constraint::Length(2), // footer bar
        ])
        .split(area);

    // Header
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Settings",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        chunks[0],
    );

    frame.render_widget(&state.inputs[F_BASE_URL], chunks[1]);
    frame.render_widget(&state.inputs[F_TOKEN], chunks[2]);
    frame.render_widget(&state.inputs[F_PROJECT], chunks[3]);
    frame.render_widget(&state.inputs[F_SPRINT], chunks[4]);

    // Info block
    let user_settings_file = crate::config::user_settings_path();
    let templates_file = crate::config::config_dir().join("templates.yaml");
    let info_lines = vec![
        Line::from(vec![
            Span::styled("  Settings and filter preferences are configured in ", Style::default().fg(Color::DarkGray)),
            Span::styled(user_settings_file.display().to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Create ticket templates are configured in ", Style::default().fg(Color::DarkGray)),
            Span::styled(templates_file.display().to_string(), Style::default().fg(Color::Cyan)),
        ]),
    ];
    frame.render_widget(Paragraph::new(info_lines), chunks[5]);

    // Footer — error or config file path
    let footer_content = if let Some(err) = &app.error {
        Line::from(Span::styled(format!(" ⚠  {err}"), Style::default().fg(Color::Red)))
    } else {
        let path = crate::config::config_dir().join("config.yaml");
        Line::from(Span::styled(format!(" {}", path.display()), Style::default().fg(Color::DarkGray)))
    };
    frame.render_widget(
        Paragraph::new(footer_content).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        chunks[6],
    );
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn field_label(idx: usize) -> &'static str {
    match idx {
        F_BASE_URL => "[1] Base URL",
        F_TOKEN => "[2] Token / PAT",
        F_PROJECT => "[3] Default Project",
        F_SPRINT => "[4] Active Sprint ID",
        _ => "",
    }
}

fn update_field_block(ta: &mut TextArea<'static>, label: &str, focused: bool, editing: bool) {
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if focused && !editing {
        format!(" {label} — Space to edit ")
    } else {
        format!(" {label} ")
    };
    ta.set_block(Block::default().borders(Borders::ALL).title(title).border_style(border_style));
    if editing {
        ta.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    } else {
        ta.set_cursor_style(Style::default());
    }
}

fn single_line_area(value: &str) -> TextArea<'static> {
    let mut ta = TextArea::from([value]);
    ta.move_cursor(tui_textarea::CursorMove::End);
    ta
}
