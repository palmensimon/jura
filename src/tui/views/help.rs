use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Renders a centred keybindings popup over whatever is currently drawn.
pub fn draw(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(58, 90, area);
    frame.render_widget(Clear, popup);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keybindings  [?] close ");
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let sections: &[(&str, &[(&str, &str)])] = &[
        (
            "Ticket List",
            &[
                ("↑/↓  j/k", "Navigate"),
                ("Enter", "Open detail"),
                ("Space", "Checkout branch (creates if absent; assigns self if configured)"),
                ("/", "Quick search (local, no API call)"),
                ("[  ]  Tab", "Switch tab (All / Mine)"),
                ("t", "Change status"),
                ("a", "Assign / unassign self"),
                ("p", "Open new PR/MR in browser"),
                ("o", "Open ticket in browser"),
                ("f", "Filter panel"),
                ("c", "Create ticket"),
                ("r", "Refresh"),
                ("s", "Settings"),
                ("q  Ctrl+C", "Quit"),
            ],
        ),
        (
            "Ticket Detail",
            &[
                ("t", "Change status (cached, instant on repeat)"),
                ("a", "Assign / unassign self (toggles)"),
                ("Space", "Checkout branch (creates if absent; prompts for name if new)"),
                ("p", "Open new PR/MR in browser (finds local branch by ticket key)"),
                ("o", "Open ticket in browser (xdg-open)"),
                ("Esc / Backspace", "Back"),
            ],
        ),
        (
            "Filter Panel  (popup)",
            &[
                ("1 – 8", "Jump to field (from non-text fields)"),
                ("Tab / ↑↓", "Navigate fields"),
                ("←/→", "Move between options"),
                ("Space", "Toggle / cycle"),
                ("Enter", "Apply filter"),
                ("Ctrl+S", "Save as default"),
                ("Esc", "Cancel"),
            ],
        ),
        (
            "Transition Picker  (popup)",
            &[
                ("type", "Filter transitions"),
                ("↑/↓", "Navigate results"),
                ("Enter", "Apply transition"),
                ("Esc / Backspace", "Back (Backspace also clears search)"),
            ],
        ),
        (
            "Settings",
            &[
                ("1 – 9", "Jump to field (from toggle fields)"),
                ("Tab / ↑↓", "Navigate fields"),
                ("Space", "Toggle boolean fields"),
                ("Enter", "Save"),
                ("r", "Reload user_defaults.yaml and templates.yaml from disk"),
                ("Ctrl+O", "Open user_defaults.yaml in nvim"),
                ("Ctrl+T", "Open templates.yaml in nvim"),
                ("Esc", "Cancel"),
            ],
        ),
        (
            "Create Ticket",
            &[
                ("Tab", "Next field"),
                ("Ctrl+S", "Submit"),
                ("Esc", "Back"),
            ],
        ),
    ];

    // Build all lines
    let mut lines: Vec<Line> = vec![];
    for (section, bindings) in sections {
        lines.push(Line::from(Span::styled(
            format!("  {section}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        for (key, action) in *bindings {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("    {key:<18}", key = key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(*action, Style::default().fg(Color::White)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

/// Renders the global bottom status bar (1 line) in the classic `[key] action` style.
/// When `loading` is true a right-aligned "loading…" indicator is drawn.
/// When `loading` is false and `right_msg` is Some, that message is shown right-aligned instead.
pub fn draw_status_bar(frame: &mut Frame, area: Rect, hints: &[(&str, &str)], loading: bool, right_msg: Option<&str>) {
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, action)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            format!("[{key}] {action}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);

    if loading {
        let label = "loading… ";
        let w = label.len() as u16;
        if w <= area.width {
            let right = Rect { x: area.x + area.width - w, y: area.y, width: w, height: 1 };
            frame.render_widget(
                Paragraph::new(Span::styled(label, Style::default().fg(Color::DarkGray))),
                right,
            );
        }
    } else if let Some(msg) = right_msg {
        let label = format!("{msg} ");
        let w = label.chars().count() as u16;
        if w <= area.width {
            let right = Rect { x: area.x + area.width - w, y: area.y, width: w, height: 1 };
            frame.render_widget(
                Paragraph::new(Span::styled(label, Style::default().fg(Color::Green))),
                right,
            );
        }
    }
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

/// Returns the context-appropriate hints for the bottom status bar.
pub fn status_bar_hints(view_name: &str) -> &'static [(&'static str, &'static str)] {
    match view_name {
        "TicketList" => &[
            ("Space", "checkout"),
            ("t", "status"),
            ("a", "assign"),
            ("p", "PR/MR"),
            ("o", "browser"),
            ("/", "search"),
            ("f", "filter"),
            ("c", "create"),
            ("r", "refresh"),
            ("?", "help"),
        ],
        "TicketDetail" => &[
            ("t", "status"),
            ("a", "assign self"),
            ("Space", "checkout"),
            ("p", "open PR/MR"),
            ("o", "browser"),
            ("Esc", "back"),
            ("?", "help"),
        ],
        "TransitionPicker" => &[
            ("type", "filter"),
            ("↑↓", "navigate"),
            ("Enter", "apply"),
            ("Esc / ⌫", "back"),
        ],
        "FilterPanel" => &[
            ("Tab/↑↓", "navigate"),
            ("Space", "toggle"),
            ("Enter", "apply"),
            ("Ctrl+S", "save"),
            ("Esc", "cancel"),
        ],
        "Settings" => &[
            ("Tab/↑↓", "navigate"),
            ("Enter", "save"),
            ("r", "reload config files"),
            ("Ctrl+O", "edit user_defaults.yaml"),
            ("Ctrl+T", "edit templates.yaml"),
            ("Esc", "cancel"),
        ],
        "CreateTicket" => &[
            ("Tab", "next field"),
            ("Ctrl+S", "submit"),
            ("Esc", "back"),
        ],
        _ => &[("?", "help"), ("Esc", "back")],
    }
}

/// Derive a simple string name for the current view (used by status_bar_hints).
pub fn view_name(view: &crate::tui::app::AppView) -> &'static str {
    match view {
        crate::tui::app::AppView::TicketList => "TicketList",
        crate::tui::app::AppView::TicketDetail { .. } => "TicketDetail",
        crate::tui::app::AppView::TransitionPicker { .. } => "TransitionPicker",
        crate::tui::app::AppView::CreateTicket => "CreateTicket",
        crate::tui::app::AppView::Settings => "Settings",
        crate::tui::app::AppView::FilterPanel => "FilterPanel",
    }
}

/// Split area into [content, status_bar].
pub fn split_with_bar(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);
    (chunks[0], chunks[1])
}
