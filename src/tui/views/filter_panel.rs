use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use tui_textarea::TextArea;

use crate::config::TeamEntry;
use crate::tui::app::{App, FilterState, SortBy, SortDir};

// ── Row enum ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveRow {
    TextSearch,
    Status,
    HiddenStatus,
    Component,
    Labels,
    Team,
    AssignedToMe,
    SprintActive,
    SortBy,
    SortDir,
}

const ROW_COUNT: usize = 10;

impl ActiveRow {
    fn index(self) -> usize {
        match self {
            ActiveRow::TextSearch => 0,
            ActiveRow::Status => 1,
            ActiveRow::HiddenStatus => 2,
            ActiveRow::Component => 3,
            ActiveRow::Labels => 4,
            ActiveRow::Team => 5,
            ActiveRow::AssignedToMe => 6,
            ActiveRow::SprintActive => 7,
            ActiveRow::SortBy => 8,
            ActiveRow::SortDir => 9,
        }
    }

    fn from_index(i: usize) -> Self {
        match i % ROW_COUNT {
            0 => ActiveRow::TextSearch,
            1 => ActiveRow::Status,
            2 => ActiveRow::HiddenStatus,
            3 => ActiveRow::Component,
            4 => ActiveRow::Labels,
            5 => ActiveRow::Team,
            6 => ActiveRow::AssignedToMe,
            7 => ActiveRow::SprintActive,
            8 => ActiveRow::SortBy,
            _ => ActiveRow::SortDir,
        }
    }
}

// ── State ─────────────────────────────────────────────────────────────────────

pub struct FilterPanelState {
    pub text_input: TextArea<'static>,
    pub status_cursor: usize,
    pub hidden_status_cursor: usize,
    pub component_cursor: usize,
    pub labels_cursor: usize,
    pub team_cursor: usize,
    pub assigned_to_me: bool,
    pub sprint_active_only: bool,
    pub sort_by: SortBy,
    pub sort_dir: SortDir,
    pub selected_statuses: Vec<String>,
    pub hidden_statuses: Vec<String>,
    pub selected_labels: Vec<String>,
    pub selected_component: Option<String>,
    pub selected_team: Option<String>,
    pub active_row: ActiveRow,
    pub text_editing: bool,
}

impl FilterPanelState {
    pub fn new(filter: &FilterState) -> Self {
        let mut text_input = TextArea::default();
        text_input.set_placeholder_text("Search key or summary…");
        if !filter.text_search.is_empty() {
            text_input = TextArea::from([filter.text_search.as_str()]);
            text_input.move_cursor(tui_textarea::CursorMove::End);
        }

        Self {
            text_input,
            status_cursor: 0,
            hidden_status_cursor: 0,
            component_cursor: 0,
            labels_cursor: 0,
            team_cursor: 0,
            assigned_to_me: filter.assigned_to_me,
            sprint_active_only: filter.sprint_active_only,
            sort_by: filter.sort_by,
            sort_dir: filter.sort_dir,
            selected_statuses: filter.selected_statuses.clone(),
            hidden_statuses: filter.hidden_statuses.clone(),
            selected_labels: filter.labels.clone(),
            selected_component: filter.component.clone(),
            selected_team: filter.team.clone(),
            active_row: ActiveRow::TextSearch,
            text_editing: false,
        }
    }

    pub fn apply_to_filter(&self, base: &FilterState) -> FilterState {
        let text_search = self
            .text_input
            .lines()
            .first()
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string();

        FilterState {
            project: base.project.clone(),
            component: self.selected_component.clone(),
            selected_statuses: self.selected_statuses.clone(),
            hidden_statuses: self.hidden_statuses.clone(),
            text_search,
            labels: self.selected_labels.clone(),
            team: self.selected_team.clone(),
            sprint_active_only: self.sprint_active_only,
            assigned_to_me: self.assigned_to_me,
            sort_by: self.sort_by,
            sort_dir: self.sort_dir,
        }
    }

    fn next_row(&mut self) {
        self.text_editing = false;
        self.active_row = ActiveRow::from_index(self.active_row.index() + 1);
    }

    fn prev_row(&mut self) {
        self.text_editing = false;
        self.active_row = ActiveRow::from_index(self.active_row.index() + ROW_COUNT - 1);
    }
}

// ── Result ────────────────────────────────────────────────────────────────────

pub enum FilterPanelResult {
    Apply(FilterState),
    Save(FilterState),
    Cancel,
}

// ── Key handling ──────────────────────────────────────────────────────────────

pub fn handle_key(
    app: &mut App,
    state: &mut FilterPanelState,
    key: KeyEvent,
) -> Option<FilterPanelResult> {
    // Number shortcuts: always available unless actively typing in a text field
    if !state.text_editing {
        let target = match key.code {
            KeyCode::Char('1') => Some(ActiveRow::TextSearch),
            KeyCode::Char('2') => Some(ActiveRow::Status),
            KeyCode::Char('3') => Some(ActiveRow::HiddenStatus),
            KeyCode::Char('4') => Some(ActiveRow::Component),
            KeyCode::Char('5') => Some(ActiveRow::Labels),
            KeyCode::Char('6') => Some(ActiveRow::Team),
            KeyCode::Char('7') => Some(ActiveRow::AssignedToMe),
            KeyCode::Char('8') => Some(ActiveRow::SprintActive),
            KeyCode::Char('9') => Some(ActiveRow::SortBy),
            _ => None,
        };
        if let Some(row) = target {
            state.active_row = row;
            return None;
        }
    }

    match key.code {
        KeyCode::Esc => {
            if state.text_editing {
                state.text_editing = false;
                return None;
            }
            return Some(FilterPanelResult::Cancel);
        }
        KeyCode::Tab => {
            state.next_row();
            return None;
        }
        KeyCode::BackTab => {
            state.prev_row();
            return None;
        }
        KeyCode::Up if !state.text_editing => {
            state.prev_row();
            return None;
        }
        KeyCode::Down if !state.text_editing => {
            state.next_row();
            return None;
        }
        KeyCode::Enter if key.modifiers.is_empty() => {
            if state.text_editing {
                state.text_editing = false;
                return None;
            }
            return Some(FilterPanelResult::Apply(state.apply_to_filter(&app.filter)));
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Some(FilterPanelResult::Save(state.apply_to_filter(&app.filter)));
        }
        _ => {}
    }

    match state.active_row {
        ActiveRow::TextSearch => {
            if state.text_editing {
                state.text_input.input(key);
            } else if key.code == KeyCode::Char(' ') {
                state.text_editing = true;
            }
        }
        ActiveRow::Status => {
            handle_multiselect_key(
                key,
                &app.available_statuses,
                &mut state.status_cursor,
                &mut state.selected_statuses,
            );
        }
        ActiveRow::HiddenStatus => {
            handle_multiselect_key(
                key,
                &app.hidden_status_options,
                &mut state.hidden_status_cursor,
                &mut state.hidden_statuses,
            );
        }
        ActiveRow::Component => {
            handle_component_key(
                key,
                &app.available_components,
                &mut state.component_cursor,
                &mut state.selected_component,
            );
        }
        ActiveRow::Labels => {
            handle_multiselect_key(
                key,
                &app.config.defaults.visible_labels,
                &mut state.labels_cursor,
                &mut state.selected_labels,
            );
        }
        ActiveRow::Team => {
            handle_team_key(
                key,
                &app.config.defaults.visible_teams,
                &mut state.team_cursor,
                &mut state.selected_team,
            );
        }
        ActiveRow::AssignedToMe => {
            if key.code == KeyCode::Char(' ') {
                state.assigned_to_me = !state.assigned_to_me;
            }
        }
        ActiveRow::SprintActive => {
            if key.code == KeyCode::Char(' ') {
                state.sprint_active_only = !state.sprint_active_only;
            }
        }
        ActiveRow::SortBy => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                state.sort_by = state.sort_by.next();
            }
        }
        ActiveRow::SortDir => {
            if matches!(key.code, KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right) {
                state.sort_dir = state.sort_dir.next();
            }
        }
    }

    None
}

fn handle_multiselect_key(
    key: KeyEvent,
    options: &[String],
    cursor: &mut usize,
    selected: &mut Vec<String>,
) {
    if options.is_empty() {
        return;
    }
    match key.code {
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor + 1 < options.len() {
                *cursor += 1;
            }
        }
        KeyCode::Char(' ') => {
            if let Some(opt) = options.get(*cursor) {
                if selected.contains(opt) {
                    selected.retain(|s| s != opt);
                } else {
                    selected.push(opt.clone());
                }
            }
        }
        _ => {}
    }
}

fn handle_team_key(
    key: KeyEvent,
    teams: &[TeamEntry],
    cursor: &mut usize,
    selected: &mut Option<String>,
) {
    let total = teams.len() + 1;
    match key.code {
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor + 1 < total {
                *cursor += 1;
            }
        }
        KeyCode::Char(' ') => {
            if *cursor == 0 {
                *selected = None;
            } else {
                *selected = teams.get(*cursor - 1).map(|t| t.id.clone());
            }
        }
        _ => {}
    }
}

fn handle_component_key(
    key: KeyEvent,
    options: &[String],
    cursor: &mut usize,
    selected: &mut Option<String>,
) {
    // slot 0 = "(all)", slots 1..N = options
    let total = options.len() + 1;
    match key.code {
        KeyCode::Left => {
            if *cursor > 0 {
                *cursor -= 1;
            }
        }
        KeyCode::Right => {
            if *cursor + 1 < total {
                *cursor += 1;
            }
        }
        KeyCode::Char(' ') => {
            if *cursor == 0 {
                *selected = None;
            } else {
                *selected = options.get(*cursor - 1).cloned();
            }
        }
        _ => {}
    }
}

// ── Drawing ───────────────────────────────────────────────────────────────────

/// Compute a centered popup rect: `percent_x` / `percent_y` of the given area.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_width = area.width * percent_x / 100;
    let popup_height = area.height * percent_y / 100;
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    Rect {
        x,
        y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    }
}

pub fn draw(app: &App, state: &mut FilterPanelState, frame: &mut Frame, area: Rect) {
    // Render as a centered popup over whatever is behind it
    let popup = centered_rect(70, 92, area);

    // Clear the popup area first so content behind doesn't bleed through
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Filter ");
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // text search
            Constraint::Length(5), // status
            Constraint::Length(5), // hidden statuses
            Constraint::Length(5), // component
            Constraint::Length(5), // labels
            Constraint::Length(5), // team
            Constraint::Length(3), // assigned_to_me
            Constraint::Length(3), // sprint_active
            Constraint::Length(3), // sort
            Constraint::Min(0),    // padding
            Constraint::Length(1), // footer
        ])
        .split(inner);

    // ── Text search ───────────────────────────────────────────────────────────
    update_textarea_block(
        &mut state.text_input,
        " [1] Text search ",
        state.active_row == ActiveRow::TextSearch,
        state.text_editing && state.active_row == ActiveRow::TextSearch,
    );
    frame.render_widget(&state.text_input, chunks[0]);

    // ── Status multi-select ───────────────────────────────────────────────────
    if !app.available_statuses.is_empty() && state.status_cursor >= app.available_statuses.len() {
        state.status_cursor = app.available_statuses.len() - 1;
    }
    frame.render_widget(
        Paragraph::new(option_list_text(
            &app.available_statuses,
            &state.selected_statuses,
            state.status_cursor,
            state.active_row == ActiveRow::Status,
            false,
        ))
        .block(focused_block(" [2] Status ", state.active_row == ActiveRow::Status))
        .wrap(Wrap { trim: false }),
        chunks[1],
    );

    // ── Hidden statuses multi-select ──────────────────────────────────────────
    if !app.hidden_status_options.is_empty() && state.hidden_status_cursor >= app.hidden_status_options.len() {
        state.hidden_status_cursor = app.hidden_status_options.len() - 1;
    }
    frame.render_widget(
        Paragraph::new(option_list_text(
            &app.hidden_status_options,
            &state.hidden_statuses,
            state.hidden_status_cursor,
            state.active_row == ActiveRow::HiddenStatus,
            false,
        ))
        .block(focused_block(" [3] Hidden statuses ", state.active_row == ActiveRow::HiddenStatus))
        .wrap(Wrap { trim: false }),
        chunks[2],
    );

    // ── Component single-select ───────────────────────────────────────────────
    let comp_total = app.available_components.len() + 1;
    if state.component_cursor >= comp_total {
        state.component_cursor = 0;
    }
    let comp_options: Vec<String> = std::iter::once("(all)".to_string())
        .chain(app.available_components.iter().cloned())
        .collect();
    let comp_selected: Vec<String> = match &state.selected_component {
        None => vec!["(all)".to_string()],
        Some(c) => vec![c.clone()],
    };
    frame.render_widget(
        Paragraph::new(option_list_text(
            &comp_options,
            &comp_selected,
            state.component_cursor,
            state.active_row == ActiveRow::Component,
            true,
        ))
        .block(focused_block(" [4] Component ", state.active_row == ActiveRow::Component))
        .wrap(Wrap { trim: false }),
        chunks[3],
    );

    // ── Labels multi-select ───────────────────────────────────────────────────
    let visible_labels = &app.config.defaults.visible_labels;
    if !visible_labels.is_empty() && state.labels_cursor >= visible_labels.len() {
        state.labels_cursor = visible_labels.len() - 1;
    }
    frame.render_widget(
        Paragraph::new(option_list_text(
            visible_labels,
            &state.selected_labels,
            state.labels_cursor,
            state.active_row == ActiveRow::Labels,
            false,
        ))
        .block(focused_block(" [5] Labels ", state.active_row == ActiveRow::Labels))
        .wrap(Wrap { trim: false }),
        chunks[4],
    );

    // ── Team single-select ────────────────────────────────────────────────────
    let visible_teams = &app.config.defaults.visible_teams;
    let team_total = visible_teams.len() + 1;
    if state.team_cursor >= team_total {
        state.team_cursor = 0;
    }
    let team_options: Vec<String> = std::iter::once("(all)".to_string())
        .chain(visible_teams.iter().map(|t| t.name.clone()))
        .collect();
    let team_selected: Vec<String> = match &state.selected_team {
        None => vec!["(all)".to_string()],
        Some(id) => visible_teams.iter()
            .find(|t| &t.id == id)
            .map(|t| vec![t.name.clone()])
            .unwrap_or_default(),
    };
    frame.render_widget(
        Paragraph::new(option_list_text(
            &team_options,
            &team_selected,
            state.team_cursor,
            state.active_row == ActiveRow::Team,
            true,
        ))
        .block(focused_block(" [6] Team ", state.active_row == ActiveRow::Team))
        .wrap(Wrap { trim: false }),
        chunks[5],
    );

    // ── Bool toggles ──────────────────────────────────────────────────────────
    draw_toggle(frame, chunks[6], "Assigned to me", 7, state.assigned_to_me,
        state.active_row == ActiveRow::AssignedToMe);
    draw_toggle(frame, chunks[7], "Sprint active only", 8, state.sprint_active_only,
        state.active_row == ActiveRow::SprintActive);

    // ── Sort ──────────────────────────────────────────────────────────────────
    let sort_focused = matches!(state.active_row, ActiveRow::SortBy | ActiveRow::SortDir);
    let sort_line = Line::from(vec![
        Span::styled(" Sort by: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            state.sort_by.label(),
            if state.active_row == ActiveRow::SortBy {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        ),
        Span::styled("   Direction: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            state.sort_dir.label(),
            if state.active_row == ActiveRow::SortDir {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        ),
    ]);
    frame.render_widget(
        Paragraph::new(sort_line).block(focused_block(" [9] Sort ", sort_focused)),
        chunks[8],
    );

    // ── Footer ────────────────────────────────────────────────────────────────
    let footer_content = if let Some(err) = &app.error {
        Line::from(Span::styled(format!(" ⚠  {err}"), Style::default().fg(Color::Red)))
    } else {
        Line::from(Span::styled(
            format!(" {} tickets loaded", app.active_tab().issues.len()),
            Style::default().fg(Color::DarkGray),
        ))
    };
    frame.render_widget(Paragraph::new(footer_content), chunks[10]);
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn focused_block(title: &str, active: bool) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if active {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn update_textarea_block(ta: &mut TextArea<'static>, label: &str, focused: bool, editing: bool) {
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if focused && !editing {
        format!("{label} Space to edit ")
    } else {
        label.to_string()
    };
    ta.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(border_style),
    );
    if editing {
        ta.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    } else {
        ta.set_cursor_style(Style::default());
    }
}

fn option_list_text(
    options: &[String],
    selected: &[String],
    cursor: usize,
    focused: bool,
    single_select: bool,
) -> Text<'static> {
    if options.is_empty() {
        return Text::from(Line::from(Span::styled(
            " loading…",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let mut spans = vec![];
    for (i, opt) in options.iter().enumerate() {
        let is_selected = selected.contains(opt);
        let is_cursor = focused && i == cursor;

        let marker = if single_select {
            if is_selected { "●" } else { "○" }
        } else if is_selected {
            "x"
        } else {
            " "
        };

        let style = if is_cursor {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        if single_select {
            spans.push(Span::styled(format!("{marker} {opt}  "), style));
        } else {
            spans.push(Span::styled(format!("[{marker}] {opt}  "), style));
        }
    }

    Text::from(Line::from(spans))
}

fn draw_toggle(frame: &mut Frame, area: Rect, label: &str, number: u8, value: bool, active: bool) {
    let border_style = if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let check = if value { "x" } else { " " };
    let color = if value { Color::Green } else { Color::DarkGray };
    let line = Line::from(vec![
        Span::styled(
            format!("[{check}]"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::raw(label.to_string()),
        if active {
            Span::styled("  Space to toggle", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        },
    ]);
    frame.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" [{number}] {label} "))
                .border_style(border_style),
        ),
        area,
    );
}

