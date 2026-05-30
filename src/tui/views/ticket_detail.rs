use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use tui_textarea::TextArea;

use crate::{
    git::{branch_name, find_branch_for_ticket, new_pr_url},
    jira::Issue,
    tui::app::{App, AppView},
};

// ── State ─────────────────────────────────────────────────────────────────────

pub struct DetailState {
    pub branch_editing: bool,
    pub branch_input: TextArea<'static>,
    pub checkout_issue: Option<Issue>,
}

impl DetailState {
    pub fn new() -> Self {
        Self {
            branch_editing: false,
            branch_input: TextArea::default(),
            checkout_issue: None,
        }
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

/// Handle keys when the branch name editor popup is active.
/// Can be called from any view (list or detail).
pub fn handle_branch_editor_key(app: &mut App, state: &mut DetailState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            state.branch_editing = false;
            state.checkout_issue = None;
        }
        KeyCode::Enter if key.modifiers.is_empty() => {
            let branch = state
                .branch_input
                .lines()
                .first()
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !branch.is_empty() {
                if let Some(issue) = state.checkout_issue.take() {
                    app.spawn_checkout(branch, &issue);
                }
            }
            state.branch_editing = false;
        }
        _ => {
            state.branch_input.input(key);
        }
    }
}

pub fn handle_key(app: &mut App, state: &mut DetailState, key: KeyEvent) {
    if state.branch_editing {
        handle_branch_editor_key(app, state, key);
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Backspace => {
            app.view = AppView::TicketList;
        }
        KeyCode::Char('a') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let issue = issue.as_ref().clone();
                app.toggle_assignment(&issue);
            }
        }
        KeyCode::Char('o') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let url = format!("{}/browse/{}", app.config.jira.base_url, issue.key);
                let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
            }
        }
        KeyCode::Char('p') => {
            if let AppView::TicketDetail { issue } = &app.view {
                match find_branch_for_ticket(&issue.key) {
                    None => app.error = Some(format!("No local branch found for {}", issue.key)),
                    Some(branch) => match new_pr_url(&branch) {
                        None => app.error = Some("Could not build PR URL — unknown remote or no origin".to_string()),
                        Some(url) => { let _ = std::process::Command::new("xdg-open").arg(&url).spawn(); }
                    },
                }
            }
        }
        KeyCode::Char(' ') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let issue = issue.as_ref().clone();
                if let Some(branch) = find_branch_for_ticket(&issue.key) {
                    app.spawn_checkout(branch, &issue);
                } else {
                    let suggested = branch_name(&issue.key, issue.summary());
                    let mut ta = TextArea::from([suggested.as_str()]);
                    ta.move_cursor(tui_textarea::CursorMove::End);
                    state.branch_input = ta;
                    state.checkout_issue = Some(issue);
                    state.branch_editing = true;
                }
            }
        }
        _ => {}
    }
}

// ── Drawing ───────────────────────────────────────────────────────────────────

pub fn draw(app: &App, state: &mut DetailState, frame: &mut Frame, area: Rect) {
    // Also renders as background when TransitionPicker is overlaid on top
    let issue = match &app.view {
        AppView::TicketDetail { issue } | AppView::TransitionPicker { issue } => issue.as_ref(),
        _ => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(6), // metadata
            Constraint::Min(0),    // description
        ])
        .split(area);

    draw_header(issue, frame, chunks[0]);
    draw_metadata(issue, frame, chunks[1]);
    draw_description(issue, frame, chunks[2]);

    if state.branch_editing {
        draw_branch_editor(state, frame, area);
    }
}

/// Renders the single bottom status bar row for the ticket detail view.
pub fn draw_bar(app: &App, state: &DetailState, frame: &mut Frame, area: Rect) {
    if state.branch_editing {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " Enter to create branch  Esc to cancel",
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    if let Some(err) = &app.error {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" Error: {err}"),
                Style::default().fg(Color::Red),
            )),
            area,
        );
        return;
    }
    if let Some(msg) = &app.status_msg {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" {msg}"),
                Style::default().fg(Color::Green),
            )),
            area,
        );
        return;
    }

    let hints: &[(&str, &str)] = &[
        ("t", "status"),
        ("Space", "checkout"),
        ("o", "browser"),
        ("?", "help"),
        ("Esc", "back"),
    ];

    let mut spans = vec![Span::raw(" ")];
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 { spans.push(Span::raw("  ")); }
        spans.push(Span::styled(
            format!("[{key}] {action}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_header(issue: &Issue, frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let type_color = match issue.issue_type() {
        "Bug" => Color::Red,
        "Story" => Color::Green,
        "Task" => Color::Blue,
        "Epic" => Color::Magenta,
        _ => Color::White,
    };

    let title = Line::from(vec![
        Span::styled(
            format!(" {} ", issue.key),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled(issue.issue_type(), Style::default().fg(type_color)),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::raw(issue.summary()),
    ]);

    frame.render_widget(Paragraph::new(title), inner);
}

fn draw_metadata(issue: &Issue, frame: &mut Frame, area: Rect) {
    let status_color = match issue.status() {
        "Done" | "Closed" => Color::Green,
        "In Progress" | "In Review" => Color::Yellow,
        _ => Color::White,
    };

    let meta = Text::from(vec![
        Line::from(vec![
            meta_label("Status:   "),
            Span::styled(issue.status(), Style::default().fg(status_color)),
            Span::raw("   "),
            meta_label("Priority: "),
            Span::raw(issue.priority()),
        ]),
        Line::from(vec![
            meta_label("Assignee: "),
            Span::raw(issue.assignee()),
        ]),
        Line::from(vec![
            meta_label("Components: "),
            Span::raw(issue.component_names().join(", ")),
        ]),
        Line::from(vec![
            meta_label("Labels:   "),
            Span::raw(issue.fields.labels.join(", ")),
        ]),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(Paragraph::new(meta).block(block), area);
}

fn draw_description(issue: &Issue, frame: &mut Frame, area: Rect) {
    let text = issue
        .description_text()
        .unwrap_or("(no description)")
        .to_string();

    let block = Block::default()
        .title(" Description ")
        .title_style(Style::default().fg(Color::DarkGray))
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        area,
    );
}

pub fn draw_branch_editor(state: &mut DetailState, frame: &mut Frame, area: Rect) {
    let popup_w = area.width.saturating_sub(8).min(90);
    let popup_h = 3;
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);
    state.branch_input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Branch name — Enter to create, Esc to cancel ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    state.branch_input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(&state.branch_input, popup);
}

fn meta_label(s: &str) -> Span<'_> {
    Span::styled(s, Style::default().fg(Color::DarkGray))
}
