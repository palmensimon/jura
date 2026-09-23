use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    config::save_settings,
    jira::Board,
    tui::app::{App, AppEvent, AppView},
};

pub struct BoardPickerState {
    pub search: String,
    pub selected: usize,
    pub prev_view: AppView,
}

impl BoardPickerState {
    pub fn new(prev_view: AppView) -> Self {
        Self { search: String::new(), selected: 0, prev_view }
    }
}

/// Row 0 is always the pinned "No board" option; rows 1.. are the (optionally filtered) boards.
fn filtered_boards<'a>(state: &BoardPickerState, boards: &'a [Board]) -> Vec<&'a Board> {
    if state.search.is_empty() {
        return boards.iter().collect();
    }
    let q = state.search.to_lowercase();
    boards
        .iter()
        .filter(|b| b.name.to_lowercase().contains(&q) || b.id.to_string().contains(&q))
        .collect()
}

fn clamp_selected(state: &mut BoardPickerState, boards: &[Board]) {
    let total = 1 + filtered_boards(state, boards).len();
    if state.selected >= total {
        state.selected = total.saturating_sub(1);
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut BoardPickerState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.view = state.prev_view.clone();
        }
        KeyCode::Backspace => {
            if state.search.is_empty() {
                app.view = state.prev_view.clone();
            } else {
                state.search.pop();
                state.selected = 0;
            }
        }
        KeyCode::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
        }
        KeyCode::Down => {
            let total = 1 + filtered_boards(state, &app.available_boards).len();
            if state.selected + 1 < total {
                state.selected += 1;
            }
        }
        KeyCode::Enter => {
            let filtered = filtered_boards(state, &app.available_boards);
            let total = 1 + filtered.len();
            if total == 0 {
                return;
            }
            let idx = state.selected.min(total - 1);
            let (new_board_id, resolved): (Option<u64>, Option<Board>) = if idx == 0 {
                (None, None)
            } else {
                let b = filtered[idx - 1].clone();
                (Some(b.id), Some(b))
            };

            let mut new_cfg = (*app.config).clone();
            new_cfg.defaults.board_id = new_board_id;
            let tx = app.event_tx.clone();
            let cfg_for_save = new_cfg.clone();
            tokio::spawn(async move {
                match save_settings(&cfg_for_save.defaults) {
                    Ok(()) => {
                        let _ = tx.send(AppEvent::BoardChanged(cfg_for_save, resolved)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await;
                    }
                }
            });

            app.view = state.prev_view.clone();
        }
        KeyCode::Char(c) => {
            state.search.push(c);
            state.selected = 0;
        }
        _ => {}
    }
    clamp_selected(state, &app.available_boards);
}

// ── Drawing ───────────────────────────────────────────────────────────────────

pub fn draw(app: &App, state: &BoardPickerState, frame: &mut Frame, area: Rect) {
    let popup_w = (area.width * 60 / 100).max(50).min(area.width);
    let filtered = filtered_boards(state, &app.available_boards);
    let list_rows = if app.available_boards.is_empty() {
        1u16
    } else {
        (1 + filtered.len()).min(14) as u16
    };
    let popup_h = (list_rows + 5).min(area.height.saturating_sub(4)).max(8);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Select board ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0), Constraint::Length(1)])
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

    let footer = Paragraph::new(Span::styled(" [↵] select   [Esc] cancel", Style::default().fg(Color::DarkGray)));

    if app.available_boards.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(" Loading boards…", Style::default().fg(Color::DarkGray))),
            chunks[1],
        );
        frame.render_widget(footer, chunks[2]);
        return;
    }

    let current_board_id = app.config.defaults.board_id;
    let mut items: Vec<ListItem> = vec![ListItem::new(Line::from(vec![
        Span::styled(
            "— No board —",
            Style::default().fg(if current_board_id.is_none() { Color::White } else { Color::DarkGray }),
        ),
        if current_board_id.is_none() {
            Span::styled(" ← current", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        },
    ]))];
    items.extend(filtered.iter().map(|b| {
        let is_current = current_board_id == Some(b.id);
        ListItem::new(Line::from(vec![
            Span::styled(b.name.as_str(), Style::default().fg(Color::White)),
            Span::styled(format!("  #{}", b.id), Style::default().fg(Color::DarkGray)),
            if is_current {
                Span::styled(" ← current", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            },
        ]))
    }));

    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let selected = state.selected.min(1 + filtered.len().saturating_sub(1));
    let mut list_state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, chunks[1], &mut list_state);
    frame.render_widget(footer, chunks[2]);
}
