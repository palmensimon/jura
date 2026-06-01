use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    jira::Transition,
    tui::app::{App, AppEvent, AppView},
};

pub struct TransitionState {
    pub selected: usize,
    pub search: String,
    pub loading: bool,
    pub return_to_list: bool,
}

impl TransitionState {
    pub fn new() -> Self {
        Self { selected: 0, search: String::new(), loading: false, return_to_list: false }
    }

    pub fn filtered<'a>(&self, transitions: &'a [Transition]) -> Vec<&'a Transition> {
        if self.search.is_empty() {
            return transitions.iter().collect();
        }
        let q = self.search.to_lowercase();
        transitions
            .iter()
            .filter(|t| t.to.name.to_lowercase().contains(&q))
            .collect()
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut TransitionState, key: KeyEvent) {
    if state.loading {
        return;
    }
    match key.code {
        KeyCode::Esc => {
            go_back(app, state);
        }
        KeyCode::Backspace => {
            if state.search.is_empty() {
                go_back(app, state);
            } else {
                state.search.pop();
                clamp_selected(state, &app.available_transitions);
            }
        }
        KeyCode::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
        }
        KeyCode::Down => {
            let n = state.filtered(&app.available_transitions).len();
            if n > 0 && state.selected + 1 < n {
                state.selected += 1;
            }
        }
        KeyCode::Enter => {
            let filtered = state.filtered(&app.available_transitions);
            if let Some(transition) = filtered.get(state.selected) {
                let transition_id = transition.id.clone();
                let to_status = transition.to.name.clone();
                let key_str = issue_key(app).to_string();
                let client = app.client.clone();
                let tx = app.event_tx.clone();
                let sprint_triggers = app.config.defaults.sprint_on_transition.clone();
                state.loading = true;
                tokio::spawn(async move {
                    match client.do_transition(&key_str, &transition_id).await {
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await;
                        }
                        Ok(()) => {
                            let _ = tx.send(AppEvent::TransitionApplied(key_str.clone())).await;
                            if sprint_triggers.iter().any(|s| s.eq_ignore_ascii_case(&to_status)) {
                                let _ = client.move_to_active_sprint(&key_str).await;
                            }
                            match client.get_issue(&key_str).await {
                                Ok(issue) => {
                                    let _ = tx.send(AppEvent::IssueReloaded(issue)).await;
                                }
                                Err(_) => {}
                            }
                        }
                    }
                });
            }
        }
        KeyCode::Char(c) => {
            state.search.push(c);
            state.selected = 0;
        }
        _ => {}
    }
}

fn go_back(app: &mut App, state: &TransitionState) {
    if state.return_to_list {
        app.view = AppView::TicketList;
    } else if let AppView::TransitionPicker { issue } = &app.view {
        let issue = issue.clone();
        app.view = AppView::TicketDetail { issue };
    }
}

fn clamp_selected(state: &mut TransitionState, transitions: &[Transition]) {
    let n = state.filtered(transitions).len();
    if n == 0 {
        state.selected = 0;
    } else {
        state.selected = state.selected.min(n - 1);
    }
}

fn issue_key(app: &App) -> &str {
    if let AppView::TransitionPicker { issue } = &app.view {
        &issue.key
    } else {
        ""
    }
}

// ── Drawing ───────────────────────────────────────────────────────────────────

pub fn draw(app: &App, state: &mut TransitionState, frame: &mut Frame, area: Rect) {
    let AppView::TransitionPicker { issue } = &app.view else {
        return;
    };

    let filtered = state.filtered(&app.available_transitions);

    // Size popup to fit the filtered list
    let list_rows = if app.available_transitions.is_empty() || filtered.is_empty() {
        1u16
    } else {
        filtered.len().min(14) as u16
    };
    let popup_h = (list_rows + 4).min(area.height.saturating_sub(4)).max(7);
    let popup_w = (area.width * 70 / 100).max(60).min(area.width);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} — change status ", issue.key))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // search line + bottom border
            Constraint::Min(0),    // results
        ])
        .split(inner);

    // Search bar
    let search_line = if state.search.is_empty() {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(" type to filter…", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(state.search.clone(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
        ])
    };
    frame.render_widget(
        Paragraph::new(search_line).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        chunks[0],
    );

    // Results list
    if state.loading {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Applying…",
                Style::default().fg(Color::Yellow),
            )),
            chunks[1],
        );
        return;
    }

    if app.available_transitions.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Loading transitions…",
                Style::default().fg(Color::DarkGray),
            )),
            chunks[1],
        );
        return;
    }

    if filtered.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No matches",
                Style::default().fg(Color::DarkGray),
            )),
            chunks[1],
        );
        return;
    }

    let current_status = issue.status();
    let items: Vec<ListItem> = filtered
        .iter()
        .map(|t| {
            let is_current = t.to.name == current_status;
            let status_color = match t.to.name.as_str() {
                "Done" | "Closed" | "Resolved" => Color::Green,
                "In Progress" | "In Review" => Color::Yellow,
                "To Do" | "Open" => Color::DarkGray,
                _ => Color::White,
            };
            let line = Line::from(vec![
                Span::styled(
                    t.to.name.as_str(),
                    Style::default().fg(if is_current { Color::DarkGray } else { status_color }),
                ),
                if is_current {
                    Span::styled(" ← current", Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw("")
                },
            ]);
            ListItem::new(line)
        })
        .collect();

    let selected = state.selected.min(filtered.len().saturating_sub(1));
    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, chunks[1], &mut list_state);
}
