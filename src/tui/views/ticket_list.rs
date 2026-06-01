use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::tui::app::{App, AppView, Tab};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.active_tab().local_search_active {
        handle_local_search_key(app, key);
        return;
    }

    match key.code {
        KeyCode::Up | KeyCode::Char('k') => app.move_selection_up(),
        KeyCode::Down | KeyCode::Char('j') => app.move_selection_down(),
        KeyCode::Tab | KeyCode::Char(']') => {
            let next = app.tab.next();
            app.switch_tab(next);
        }
        KeyCode::BackTab | KeyCode::Char('[') => {
            let prev = app.tab.prev();
            app.switch_tab(prev);
        }
        KeyCode::Enter => {
            if let Some(issue) = app.selected_issue() {
                let issue = issue.clone();
                app.view = AppView::TicketDetail {
                    issue: Box::new(issue),
                };
            }
        }
        KeyCode::Char('r') => {
            let branch = crate::git::current_branch().ok();
            app.current_branch_key = branch.as_deref().and_then(crate::git::extract_ticket_key);
            app.current_branch_name = branch;
            app.trigger_load_tab(Tab::All);
            app.trigger_load_tab(Tab::Mine);
        }
        KeyCode::Char('/') => {
            let ts = app.active_tab_mut();
            ts.local_search_active = true;
            ts.local_search.clear();
            ts.selected_row = 0;
        }
        _ => {}
    }
}

fn handle_local_search_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            let ts = app.active_tab_mut();
            ts.local_search_active = false;
            ts.local_search.clear();
            ts.selected_row = 0;
        }
        KeyCode::Enter => {
            app.active_tab_mut().local_search_active = false;
        }
        KeyCode::Backspace => {
            let ts = app.active_tab_mut();
            ts.local_search.pop();
            ts.selected_row = 0;
        }
        KeyCode::Char(c) => {
            let ts = app.active_tab_mut();
            ts.local_search.push(c);
            ts.selected_row = 0;
        }
        _ => {}
    }
}

pub fn draw(app: &mut App, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(area);

    draw_header(app, frame, chunks[0]);

    let ts = app.active_tab();
    if ts.loading && ts.issues.is_empty() {
        draw_loading(frame, chunks[1]);
        return;
    }

    draw_table(app, frame, chunks[1]);
}

fn draw_loading(frame: &mut Frame, area: Rect) {
    let label = "Fetching tickets…";
    let y = area.y + area.height / 2;
    let x = area.x + area.width.saturating_sub(label.len() as u16) / 2;
    let rect = Rect::new(x, y, label.len() as u16, 1);
    frame.render_widget(
        Paragraph::new(Span::styled(label, Style::default().fg(Color::DarkGray))),
        rect,
    );
}

/// Draws the single bottom bar row for the ticket list view.
pub fn draw_bar(app: &App, frame: &mut Frame, area: Rect) {
    let ts = app.active_tab();
    let content = if ts.local_search_active {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(ts.local_search.clone(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("  ({} matches — Esc clear, Enter confirm)", ts.visible_issues().len()),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if !ts.local_search.is_empty() {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(ts.local_search.clone(), Style::default().fg(Color::White)),
            Span::styled(
                format!("  ({} matches — / to edit, Esc clear)", ts.visible_issues().len()),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if let Some(err) = &app.error {
        Line::from(Span::styled(format!(" Error: {err}"), Style::default().fg(Color::Red)))
    } else if let Some(msg) = &app.status_msg {
        Line::from(Span::styled(format!(" {msg}"), Style::default().fg(Color::Green)))
    } else {
        let mut spans = vec![Span::raw(" ")];
        for (i, (key, action)) in super::help::status_bar_hints(&AppView::TicketList).iter().enumerate() {
            if i > 0 { spans.push(Span::raw("  ")); }
            spans.push(Span::styled(
                format!("[{key}] {action}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if let Some(issue) = app.selected_issue() {
            if app.current_branch_key.as_deref() == Some(issue.key.as_str()) {
                if let Some(branch) = &app.current_branch_name {
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        format!("● {branch}"),
                        Style::default().fg(Color::Green),
                    ));
                }
            }
        }
        Line::from(spans)
    };

    frame.render_widget(Paragraph::new(content), area);

    if ts.loading {
        let label = "loading… ";
        let w = label.len() as u16;
        if w <= area.width {
            let right = Rect { x: area.x + area.width - w, y: area.y, width: w, height: 1 };
            frame.render_widget(
                Paragraph::new(Span::styled(label, Style::default().fg(Color::DarkGray))),
                right,
            );
        }
    }
}

fn draw_header(app: &App, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let summary =format!("{} tickets", app.active_tab().issues.len());
    let right_width = (summary.chars().count() as u16 + 1).min(area.width.saturating_sub(20));

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(inner);

    let project = app.filter.project.as_deref().unwrap_or("all projects");

    let mut title_spans: Vec<Span> = vec![
        Span::styled(" jura ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("│ "),
        Span::styled(project, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
    ];
    for (i, tab) in [Tab::All, Tab::Mine].iter().enumerate() {
        if i > 0 {
            title_spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));
        }
        let style = if *tab == app.tab {
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        title_spans.push(Span::styled(tab.label(), style));
    }
    frame.render_widget(Paragraph::new(Line::from(title_spans)), chunks[0]);

    if right_width > 0 {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                summary,
                Style::default().fg(Color::DarkGray),
            ))),
            chunks[1],
        );
    }
}

fn draw_table(app: &mut App, frame: &mut Frame, area: Rect) {
    let selected_style = Style::default()
        .bg(Color::Rgb(40, 40, 60))
        .add_modifier(Modifier::BOLD);

    let header_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);

    let header = Row::new(vec![
        Cell::from("KEY").style(header_style),
        Cell::from("TYPE").style(header_style),
        Cell::from("STATUS").style(header_style),
        Cell::from("SUMMARY").style(header_style),
        Cell::from("ASSIGNEE").style(header_style),
    ])
    .height(1);

    let ts = app.active_tab();
    let visible = ts.visible_issues();
    let selected_row = ts.selected_row;
    let current_key = app.current_branch_key.as_deref();
    let rows: Vec<Row> = visible.iter().map(|issue| build_issue_row(issue, current_key)).collect();

    let widths = issue_column_widths();

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::NONE))
        .row_highlight_style(selected_style)
        .highlight_symbol("▶ ")
        .column_spacing(2);

    let mut state = TableState::default().with_selected(Some(selected_row));
    frame.render_stateful_widget(table, area, &mut state);
}

fn build_issue_row<'a>(issue: &'a crate::jira::Issue, current_branch_key: Option<&str>) -> Row<'a> {
    let type_color = match issue.issue_type() {
        "Bug" => Color::Red,
        "Story" => Color::Green,
        "Task" => Color::Blue,
        "Epic" => Color::Magenta,
        _ => Color::White,
    };
    let status_color = match issue.status() {
        "Done" | "Closed" | "Resolved" => Color::Green,
        "In Progress" | "In Review" => Color::Yellow,
        "To Do" | "Open" => Color::Blue,
        _ => Color::White,
    };
    let is_checked_out = current_branch_key == Some(issue.key.as_str());
    let key_style = if is_checked_out {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let key_text = if is_checked_out {
        format!("● {}", issue.key)
    } else {
        issue.key.clone()
    };

    Row::new(vec![
        Cell::from(key_text).style(key_style),
        Cell::from(issue.issue_type().to_string()).style(Style::default().fg(type_color)),
        Cell::from(issue.status().to_string()).style(Style::default().fg(status_color)),
        Cell::from(issue.summary().to_string()),
        Cell::from(issue.assignee().to_string()).style(Style::default().fg(Color::DarkGray)),
    ])
}

fn issue_column_widths() -> [Constraint; 5] {
    [
        Constraint::Length(14),
        Constraint::Length(10),
        Constraint::Length(14),
        Constraint::Min(30),
        Constraint::Length(18),
    ]
}
