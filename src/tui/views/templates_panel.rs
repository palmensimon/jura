use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

use crate::{
    config::save_templates,
    tui::app::{App, AppEvent},
};

pub struct TemplatesPanelState {
    pub selected_idx: usize,
    pub confirm_delete: Option<usize>,
}

impl TemplatesPanelState {
    pub fn new() -> Self {
        Self { selected_idx: 0, confirm_delete: None }
    }
}

pub enum TemplatesPanelResult {
    Selected(usize),
    Edit(usize),
    New,
    Cancel,
}

pub fn handle_key(app: &mut App, state: &mut TemplatesPanelState, key: KeyEvent) -> Option<TemplatesPanelResult> {
    if let Some(idx) = state.confirm_delete {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let mut new_list = app.templates.clone();
                if idx < new_list.len() {
                    new_list.remove(idx);
                }
                state.selected_idx = state.selected_idx.min(new_list.len().saturating_sub(1));
                state.confirm_delete = None;
                spawn_save(app, new_list);
            }
            _ => {
                state.confirm_delete = None;
            }
        }
        return None;
    }

    let templates_len = app.templates.len();
    match key.code {
        KeyCode::Esc => Some(TemplatesPanelResult::Cancel),
        KeyCode::Up | KeyCode::Char('k') => {
            if state.selected_idx > 0 {
                state.selected_idx -= 1;
            }
            None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.selected_idx + 1 < templates_len {
                state.selected_idx += 1;
            }
            None
        }
        KeyCode::Enter => {
            if templates_len > 0 {
                Some(TemplatesPanelResult::Selected(state.selected_idx))
            } else {
                None
            }
        }
        KeyCode::Char('a') => Some(TemplatesPanelResult::New),
        KeyCode::Char('e') => {
            if templates_len > 0 {
                Some(TemplatesPanelResult::Edit(state.selected_idx))
            } else {
                None
            }
        }
        KeyCode::Char('d') => {
            if templates_len > 0 {
                let mut new_list = app.templates.clone();
                let mut dup = new_list[state.selected_idx].clone();
                dup.name = format!("{} (copy)", dup.name);
                new_list.insert(state.selected_idx + 1, dup);
                spawn_save(app, new_list);
            }
            None
        }
        KeyCode::Char('x') => {
            if templates_len > 0 {
                state.confirm_delete = Some(state.selected_idx);
            }
            None
        }
        _ => None,
    }
}

fn spawn_save(app: &App, new_list: Vec<crate::config::TicketTemplate>) {
    let tx = app.event_tx.clone();
    tokio::spawn(async move {
        match save_templates(&new_list) {
            Ok(()) => {
                let _ = tx.send(AppEvent::TemplatesSaved(new_list)).await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await;
            }
        }
    });
}

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

pub fn draw(app: &App, state: &TemplatesPanelState, frame: &mut Frame, area: Rect) {
    let popup = centered_rect(55, 60, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Select Template ");
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    if app.templates.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No templates configured — press a to add one",
                Style::default().fg(Color::DarkGray),
            )),
            chunks[0],
        );
    } else {
        let items: Vec<ListItem> = app
            .templates
            .iter()
            .map(|t| {
                ListItem::new(Line::from(vec![
                    Span::styled(&t.name, Style::default().fg(Color::White)),
                    Span::styled(
                        format!("  {} / {}", t.project, t.issue_type),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(40, 40, 60))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        let mut list_state = ListState::default().with_selected(Some(state.selected_idx));
        frame.render_stateful_widget(list, chunks[0], &mut list_state);
    }

    let footer = if let Some(idx) = state.confirm_delete {
        let name = app.templates.get(idx).map(|t| t.name.as_str()).unwrap_or("");
        Line::from(Span::styled(
            format!(" Delete '{name}'?  y / any other key to cancel"),
            Style::default().fg(Color::Red),
        ))
    } else if let Some(err) = &app.error {
        Line::from(Span::styled(
            format!(" ⚠  {err}"),
            Style::default().fg(Color::Red),
        ))
    } else {
        Line::from(Span::styled(
            format!(" {} templates   [a] add  [e] edit  [d] duplicate  [x] delete", app.templates.len()),
            Style::default().fg(Color::DarkGray),
        ))
    };
    frame.render_widget(Paragraph::new(footer), chunks[1]);
}
