use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::tui::app::{App, AppEvent, AppView};

pub struct TicketSearchState {
    pub input: String,
    pub prefix_len: usize,
    pub loading: bool,
    pub prev_view: AppView,
}

impl TicketSearchState {
    pub fn new(project: Option<&str>, prev_view: AppView) -> Self {
        let prefix = match project {
            Some(p) if !p.is_empty() => format!("{p}-"),
            _ => String::new(),
        };
        let prefix_len = prefix.len();
        Self { input: prefix, prefix_len, loading: false, prev_view }
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut TicketSearchState, key: KeyEvent) {
    if state.loading {
        return;
    }
    match key.code {
        KeyCode::Esc => {
            app.view = state.prev_view.clone();
        }
        KeyCode::Backspace => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // ctrl+backspace: clear suffix only
                state.input.truncate(state.prefix_len);
            } else if state.input.len() > state.prefix_len {
                state.input.pop();
            }
        }
        KeyCode::Enter => {
            let key_str = state.input.trim().to_uppercase();
            if key_str.len() <= state.prefix_len {
                return;
            }
            state.loading = true;
            let client = app.client.clone();
            let tx = app.event_tx.clone();
            tokio::spawn(async move {
                match client.get_issue(&key_str).await {
                    Ok(issue) => {
                        let _ = tx.send(AppEvent::TicketFound(issue)).await;
                    }
                    Err(_) => {
                        let _ = tx
                            .send(AppEvent::Error(format!("Ticket {key_str} does not exist")))
                            .await;
                    }
                }
            });
        }
        KeyCode::Char(c) => {
            state.input.push(c);
        }
        _ => {}
    }
}

// ── Drawing ───────────────────────────────────────────────────────────────────

pub fn draw(state: &TicketSearchState, frame: &mut Frame, area: Rect) {
    let popup_w = 54u16.min(area.width);
    let popup_h = 5u16;
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Go to ticket ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    // Input line
    let suffix = &state.input[state.prefix_len..];
    let input_line = if state.loading {
        Line::from(vec![
            Span::styled(
                state.input.clone(),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" searching…", Style::default().fg(Color::Yellow)),
        ])
    } else {
        let prefix = &state.input[..state.prefix_len];
        Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::DarkGray)),
            Span::styled(
                suffix,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    };
    frame.render_widget(Paragraph::new(input_line), chunks[0]);

    // Empty separator line (chunk[1] is blank)

    // Hint line
    let hint = Line::from(vec![
        Span::styled("↵", Style::default().fg(Color::DarkGray)),
        Span::styled(" open  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::DarkGray)),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hint), chunks[2]);
}
