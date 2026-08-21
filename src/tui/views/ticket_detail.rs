use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use tui_textarea::TextArea;

use crate::{
    git::{branch_name, find_branches_for_ticket, list_local_branches, new_pr_url, open_url},
    jira::Issue,
    tui::app::{App, AppView},
};

// ── State ─────────────────────────────────────────────────────────────────────

pub enum BranchPickState {
    Idle,
    Editing { input: TextArea<'static>, issue: Issue },
    Picking { branches: Vec<String>, selected: usize, issue: Issue },
    SelectingBase {
        branch: String,
        issue: Issue,
        branches: Vec<String>,
        current: Option<String>,
        search: String,
        selected: usize,
    },
}

fn filter_bases<'a>(branches: &'a [String], search: &str) -> Vec<&'a String> {
    if search.is_empty() {
        return branches.iter().collect();
    }
    let q = search.to_lowercase();
    branches.iter().filter(|b| b.to_lowercase().contains(&q)).collect()
}

pub struct DetailState {
    pub branch_pick: BranchPickState,
    pub scroll: u16,
}

impl DetailState {
    pub fn new() -> Self {
        Self { branch_pick: BranchPickState::Idle, scroll: 0 }
    }

    pub fn reset_scroll(&mut self) {
        self.scroll = 0;
    }

    pub fn is_picking(&self) -> bool {
        !matches!(self.branch_pick, BranchPickState::Idle)
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

/// Handle keys when the branch name editor popup is active.
/// Can be called from any view (list or detail).
pub fn handle_branch_editor_key(app: &mut App, state: &mut DetailState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            state.branch_pick = BranchPickState::Idle;
        }
        KeyCode::Enter if key.modifiers.is_empty() => {
            if let BranchPickState::Editing { input, issue } = &state.branch_pick {
                let branch = input.lines().first().cloned().unwrap_or_default().trim().to_string();
                if !branch.is_empty() {
                    let issue = issue.clone();
                    let branches = list_local_branches();
                    let current = app.current_branch_name.clone();
                    let selected = current
                        .as_deref()
                        .and_then(|c| branches.iter().position(|b| b == c))
                        .unwrap_or(0);
                    state.branch_pick = BranchPickState::SelectingBase {
                        branch,
                        issue,
                        branches,
                        current,
                        search: String::new(),
                        selected,
                    };
                } else {
                    state.branch_pick = BranchPickState::Idle;
                }
            }
        }
        _ => {
            if let BranchPickState::Editing { input, .. } = &mut state.branch_pick {
                input.input(key);
            }
        }
    }
}

/// Handle keys when the branch picker popup is active.
pub fn handle_branch_picker_key(app: &mut App, state: &mut DetailState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            state.branch_pick = BranchPickState::Idle;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let BranchPickState::Picking { selected, .. } = &mut state.branch_pick {
                if *selected > 0 { *selected -= 1; }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if let BranchPickState::Picking { selected, branches, .. } = &mut state.branch_pick {
                if *selected + 1 < branches.len() + 1 { *selected += 1; }
            }
        }
        KeyCode::Enter => {
            if let BranchPickState::Picking { branches, selected, issue } = &state.branch_pick {
                let sel = *selected;
                if sel < branches.len() {
                    let branch = branches[sel].clone();
                    let issue = issue.clone();
                    state.branch_pick = BranchPickState::Idle;
                    app.spawn_checkout(branch, None, &issue);
                } else {
                    let issue = issue.clone();
                    let suggested = branch_name(&issue.key, issue.summary());
                    let mut ta = TextArea::from([suggested.as_str()]);
                    ta.move_cursor(tui_textarea::CursorMove::End);
                    state.branch_pick = BranchPickState::Editing { input: ta, issue };
                }
            }
        }
        _ => {}
    }
}

/// Handle keys when the base-branch picker popup is active.
pub fn handle_base_picker_key(app: &mut App, state: &mut DetailState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            state.branch_pick = BranchPickState::Idle;
        }
        KeyCode::Backspace => {
            if let BranchPickState::SelectingBase { search, branches, selected, .. } = &mut state.branch_pick {
                search.pop();
                let n = filter_bases(branches, search).len();
                if n == 0 {
                    *selected = 0;
                } else {
                    *selected = (*selected).min(n - 1);
                }
            }
        }
        KeyCode::Up => {
            if let BranchPickState::SelectingBase { selected, .. } = &mut state.branch_pick {
                if *selected > 0 { *selected -= 1; }
            }
        }
        KeyCode::Down => {
            if let BranchPickState::SelectingBase { branches, search, selected, .. } = &mut state.branch_pick {
                let n = filter_bases(branches, search).len();
                if n > 0 && *selected + 1 < n { *selected += 1; }
            }
        }
        KeyCode::Enter => {
            if let BranchPickState::SelectingBase { branch, issue, branches, search, selected, .. } = &state.branch_pick {
                let filtered = filter_bases(branches, search);
                if let Some(base) = filtered.get(*selected) {
                    let branch = branch.clone();
                    let base = (*base).clone();
                    let issue = issue.clone();
                    state.branch_pick = BranchPickState::Idle;
                    app.spawn_checkout(branch, Some(base), &issue);
                }
            }
        }
        KeyCode::Char(c) => {
            if let BranchPickState::SelectingBase { search, selected, .. } = &mut state.branch_pick {
                search.push(c);
                *selected = 0;
            }
        }
        _ => {}
    }
}

pub fn handle_key(app: &mut App, state: &mut DetailState, key: KeyEvent) {
    if matches!(state.branch_pick, BranchPickState::Editing { .. }) {
        handle_branch_editor_key(app, state, key);
        return;
    }
    if matches!(state.branch_pick, BranchPickState::Picking { .. }) {
        handle_branch_picker_key(app, state, key);
        return;
    }
    if matches!(state.branch_pick, BranchPickState::SelectingBase { .. }) {
        handle_base_picker_key(app, state, key);
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Backspace => {
            state.scroll = 0;
            app.view = AppView::TicketList;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll = state.scroll.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.scroll = state.scroll.saturating_add(1);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.scroll = state.scroll.saturating_sub(10);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.scroll = state.scroll.saturating_add(10);
        }
        KeyCode::PageUp => { state.scroll = state.scroll.saturating_sub(20); }
        KeyCode::PageDown => { state.scroll = state.scroll.saturating_add(20); }
        KeyCode::Char('a') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let issue = issue.as_ref().clone();
                app.toggle_assignment(&issue);
            }
        }
        KeyCode::Char('r') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let key = issue.key.clone();
                app.reload_issue(key);
            }
        }
        KeyCode::Char('b') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let url = format!("{}/browse/{}", app.config.jira.base_url, issue.key);
                let _ = open_url(&url, app.config.defaults.browser.as_deref());
            }
        }
        KeyCode::Char('o') => {
            if let AppView::TicketDetail { issue } = &app.view {
                if app.current_branch_key.as_deref() == Some(issue.key.as_str()) {
                    if let Some(branch) = &app.current_branch_name {
                        match new_pr_url(branch) {
                            None => app.error = Some("Could not build PR URL — unknown remote or no origin".to_string()),
                            Some(url) => { let _ = open_url(&url, app.config.defaults.browser.as_deref()); }
                        }
                    }
                } else {
                    app.error = Some("Checkout a branch for this ticket first".to_string());
                }
            }
        }
        KeyCode::Char('c') => {
            if let AppView::TicketDetail { issue } = &app.view {
                let issue = issue.as_ref().clone();
                let branches = find_branches_for_ticket(&issue.key);
                match branches.len() {
                    0 => {
                        let suggested = branch_name(&issue.key, issue.summary());
                        let mut ta = TextArea::from([suggested.as_str()]);
                        ta.move_cursor(tui_textarea::CursorMove::End);
                        state.branch_pick = BranchPickState::Editing { input: ta, issue };
                    }
                    1 => app.spawn_checkout(branches.into_iter().next().unwrap(), None, &issue),
                    _ => state.branch_pick = BranchPickState::Picking { branches, selected: 0, issue },
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
            Constraint::Length(7), // metadata
            Constraint::Min(0),    // description
        ])
        .split(area);

    draw_header(issue, frame, chunks[0]);
    draw_metadata(issue, frame, chunks[1]);
    draw_description(issue, state, frame, chunks[2]);

    match &state.branch_pick {
        BranchPickState::Editing { .. } => draw_branch_editor(state, frame, area),
        BranchPickState::Picking { .. } => draw_branch_picker(state, frame, area),
        BranchPickState::SelectingBase { .. } => draw_base_picker(state, frame, area),
        BranchPickState::Idle => {}
    }
}

/// Renders the single bottom status bar row for the ticket detail view.
pub fn draw_bar(app: &App, state: &DetailState, frame: &mut Frame, area: Rect) {
    match &state.branch_pick {
        BranchPickState::Editing { .. } => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    " Enter to create branch  Esc to cancel",
                    Style::default().fg(Color::DarkGray),
                )),
                area,
            );
            return;
        }
        BranchPickState::Picking { .. } => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    " ↑↓ to select  Enter to checkout  Esc to cancel",
                    Style::default().fg(Color::DarkGray),
                )),
                area,
            );
            return;
        }
        BranchPickState::SelectingBase { .. } => {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    " ↑↓ select base  Enter create  Esc cancel  type to filter",
                    Style::default().fg(Color::DarkGray),
                )),
                area,
            );
            return;
        }
        BranchPickState::Idle => {}
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

    let mut spans = vec![Span::raw(" ")];
    for (i, (key, action)) in super::help::status_bar_hints(&app.view).iter().enumerate() {
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
        Line::from(vec![
            meta_label("Fix Version: "),
            Span::raw(issue.fix_version_names().join(", ")),
        ]),
    ]);

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    frame.render_widget(Paragraph::new(meta).block(block), area);
}

fn draw_description(issue: &Issue, state: &DetailState, frame: &mut Frame, area: Rect) {
    let text = issue.description_text().unwrap_or("*(no description)*");

    let block = Block::default()
        .title(" Description ")
        .title_style(Style::default().fg(Color::DarkGray))
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = crate::markdown::render(text, inner.width);
    frame.render_widget(
        Paragraph::new(lines).scroll((state.scroll, 0)).wrap(Wrap { trim: false }),
        inner,
    );
}

pub fn draw_branch_editor(state: &mut DetailState, frame: &mut Frame, area: Rect) {
    let BranchPickState::Editing { input, .. } = &mut state.branch_pick else { return; };

    let popup_w = area.width.saturating_sub(8).min(90);
    let popup_h = 3;
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);
    input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Branch name — Enter to create, Esc to cancel ")
            .border_style(Style::default().fg(Color::Yellow)),
    );
    input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(&*input, popup);
}

pub fn draw_branch_picker(state: &DetailState, frame: &mut Frame, area: Rect) {
    let BranchPickState::Picking { branches, selected, .. } = &state.branch_pick else { return; };

    let total = branches.len() + 1; // +1 for "create new" entry
    let popup_h = (total as u16 + 2).min(area.height.saturating_sub(4));
    let popup_w = area.width.saturating_sub(8).min(90);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let create_idx = branches.len();
    let mut items: Vec<Line> = branches.iter().enumerate().map(|(i, b)| {
        if i == *selected {
            Line::from(Span::styled(
                format!(" ▶ {b}"),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ))
        } else {
            Line::from(Span::raw(format!("   {b}")))
        }
    }).collect();
    if *selected == create_idx {
        items.push(Line::from(Span::styled(
            " ▶ + Create new branch…",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
    } else {
        items.push(Line::from(Span::styled(
            "   + Create new branch…",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select branch — ↑↓ navigate, Enter checkout, Esc cancel ")
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(Paragraph::new(items).block(block), popup);
}

pub fn draw_base_picker(state: &DetailState, frame: &mut Frame, area: Rect) {
    let BranchPickState::SelectingBase { branch, branches, current, search, selected, .. } = &state.branch_pick else { return; };

    let filtered = filter_bases(branches, search);

    let list_rows = if filtered.is_empty() { 1u16 } else { filtered.len().min(14) as u16 };
    let popup_h = (list_rows + 4).min(area.height.saturating_sub(4)).max(7);
    let popup_w = (area.width * 70 / 100).max(60).min(area.width);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Base for {branch} — Enter to create, Esc to cancel "))
        .border_style(Style::default().fg(Color::Yellow));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // search line + bottom border
            Constraint::Min(0),    // results
        ])
        .split(inner);

    let search_line = if search.is_empty() {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(" type to filter…", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(search.clone(), Style::default().fg(Color::White)),
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

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|b| {
            let is_current = current.as_deref() == Some(b.as_str());
            let line = Line::from(vec![
                Span::styled(
                    b.as_str(),
                    Style::default().fg(if is_current { Color::DarkGray } else { Color::White }),
                ),
                if is_current {
                    Span::styled(" (current)", Style::default().fg(Color::DarkGray))
                } else {
                    Span::raw("")
                },
            ]);
            ListItem::new(line)
        })
        .collect();

    let sel = (*selected).min(filtered.len().saturating_sub(1));
    let list = List::new(items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut list_state = ListState::default().with_selected(Some(sel));
    frame.render_stateful_widget(list, chunks[1], &mut list_state);
}

fn meta_label(s: &str) -> Span<'_> {
    Span::styled(s, Style::default().fg(Color::DarkGray))
}
