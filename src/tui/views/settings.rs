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
const FIELD_COUNT: usize = 3;

pub struct SettingsState {
    inputs: [TextArea<'static>; FIELD_COUNT],
    token_revealed: bool,
    active: usize,
}

impl SettingsState {
    pub fn new(config: &Config) -> Self {
        let mut inputs = std::array::from_fn(|_| TextArea::default());
        inputs[F_BASE_URL] = single_line_area(&config.jira.base_url);
        inputs[F_TOKEN] = masked_area(&config.jira.token);
        inputs[F_PROJECT] = single_line_area(config.defaults.project.as_deref().unwrap_or(""));

        let mut state = Self { inputs, token_revealed: false, active: 0 };
        state.refresh_styles();
        state
    }

    fn refresh_styles(&mut self) {
        for (i, input) in self.inputs.iter_mut().enumerate() {
            let is_active = i == self.active;
            let (label, masked) = field_meta(i);
            input.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {label} "))
                    .border_style(if is_active {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    }),
            );
            if masked {
                input.set_mask_char('●');
            }
        }
    }

    fn reveal_token(&mut self, show: bool) {
        self.token_revealed = show;
        if show {
            self.inputs[F_TOKEN].clear_mask_char();
        } else {
            self.inputs[F_TOKEN].set_mask_char('●');
        }
    }

    fn move_to(&mut self, idx: usize) {
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
        if base_url.is_empty() {
            return Err("Base URL is required".to_string());
        }
        if token.is_empty() {
            return Err("Token / PAT is required".to_string());
        }
        let mut defaults = existing.defaults.clone();
        defaults.project = if project.is_empty() { None } else { Some(project) };
        Ok(Config {
            jira: JiraConfig { base_url, token },
            defaults,
        })
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut SettingsState, key: KeyEvent) {
    let is_save = key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL);
    if is_save {
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

    match key.code {
        KeyCode::Esc => {
            app.view = AppView::TicketList;
            app.error = None;
        }
        KeyCode::Tab => state.move_next(),
        KeyCode::BackTab => state.move_prev(),
        KeyCode::Up => state.move_prev(),
        KeyCode::Down => state.move_next(),
        KeyCode::Char('1') => state.move_to(F_BASE_URL),
        KeyCode::Char('2') => state.move_to(F_TOKEN),
        KeyCode::Char('3') => state.move_to(F_PROJECT),
        KeyCode::Char('r') if state.active == F_TOKEN && key.modifiers.is_empty() => {
            let next = !state.token_revealed;
            state.reveal_token(next);
        }
        _ => {
            state.inputs[state.active].input(key);
        }
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
            Constraint::Min(0),    // user_defaults.yaml info
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

    // Token with r·reveal hint
    let token_area = chunks[2];
    frame.render_widget(&state.inputs[F_TOKEN], token_area);
    if state.active == F_TOKEN && token_area.height >= 3 {
        let hint = if state.token_revealed { "r·hide" } else { "r·reveal" };
        let hint_rect = Rect {
            x: token_area.right().saturating_sub(hint.len() as u16 + 3),
            y: token_area.y,
            width: hint.len() as u16 + 2,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {hint} "),
                Style::default().fg(Color::DarkGray),
            )),
            hint_rect,
        );
    }

    frame.render_widget(&state.inputs[F_PROJECT], chunks[3]);

    // Info block
    let user_defaults_file = crate::config::user_defaults_path();
    let templates_file = crate::config::config_dir().join("templates.yaml");
    let info_lines = vec![
        Line::from(vec![
            Span::styled("  Defaults and filter preferences are configured in ", Style::default().fg(Color::DarkGray)),
            Span::styled(user_defaults_file.display().to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Create ticket templates are configured in ", Style::default().fg(Color::DarkGray)),
            Span::styled(templates_file.display().to_string(), Style::default().fg(Color::Cyan)),
        ]),
    ];
    frame.render_widget(Paragraph::new(info_lines), chunks[4]);

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
        chunks[5],
    );
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn field_meta(idx: usize) -> (&'static str, bool) {
    match idx {
        F_BASE_URL => ("[1] Base URL", false),
        F_TOKEN => ("[2] Token / PAT", true),
        F_PROJECT => ("[3] Default Project", false),
        _ => ("", false),
    }
}

fn single_line_area(value: &str) -> TextArea<'static> {
    let mut ta = TextArea::from([value]);
    ta.move_cursor(tui_textarea::CursorMove::End);
    ta
}

fn masked_area(value: &str) -> TextArea<'static> {
    let mut ta = single_line_area(value);
    ta.set_mask_char('●');
    ta
}
