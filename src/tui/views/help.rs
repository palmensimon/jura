use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::tui::app::AppView;

/// Renders a centred keybindings popup over whatever is currently drawn.
/// `scroll` is the number of lines scrolled from the top; it is clamped internally.
pub fn draw(frame: &mut Frame, area: Rect, scroll: u16) {
    let popup = centered_rect(58, 90, area);
    frame.render_widget(Clear, popup);

    let sections: &[(&str, &[(&str, &str)])] = &[
        (
            "Global",
            &[
                ("↑/↓  j/k", "Navigate"),
                ("Tab / ↑↓", "Navigate fields"),
                ("1 – 9", "Jump to field"),
                ("Esc", "Cancel / back"),
                ("q  Ctrl+C", "Quit"),
                ("?", "Toggle help"),
            ],
        ),
        (
            "Ticket actions  (List + Detail)",
            &[
                ("t", "Change status"),
                ("a", "Assign / unassign self"),
                ("c", "Checkout / create branch"),
                ("Shift+C", "Branch picker"),
                ("o", "Open PR/MR in browser"),
                ("b", "Open ticket in browser"),
                ("⌫", "Back  (Detail only)"),
            ],
        ),
        (
            "Ticket List",
            &[
                ("Enter", "Open detail"),
                ("/", "Quick search"),
                ("[  ]  Tab", "Switch tab"),
                ("f", "Filter panel"),
                ("n", "New ticket"),
                ("r", "Refresh"),
                ("s", "Settings"),
            ],
        ),
        (
            "Filter Panel",
            &[
                ("←/→", "Move between options"),
                ("Space", "Toggle / cycle"),
                ("Enter", "Apply filter"),
                ("Ctrl+S", "Save as default"),
            ],
        ),
        (
            "Transition Picker",
            &[
                ("type", "Filter transitions"),
                ("Enter", "Apply"),
                ("⌫", "Back + clear search"),
            ],
        ),
        (
            "Select Template",
            &[
                ("↑/↓  j/k", "Navigate"),
                ("Enter", "Select"),
                ("r", "Reload templates"),
                ("Ctrl+T", "Edit templates.yaml"),
            ],
        ),
        (
            "Settings",
            &[
                ("Space", "Toggle"),
                ("Enter", "Save"),
                ("r", "Reload config files"),
                ("Ctrl+D", "Edit user_settings.yaml"),
                ("Ctrl+T", "Edit templates.yaml"),
            ],
        ),
        (
            "Create Ticket",
            &[
                ("Ctrl+S", "Submit"),
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

    let inner_height = popup.height.saturating_sub(2);
    let total = lines.len() as u16;
    let max_scroll = total.saturating_sub(inner_height);
    let effective_scroll = scroll.min(max_scroll);

    let needs_scroll = total > inner_height;
    let title = if needs_scroll {
        " Keybindings  [?] close  ↑↓ scroll "
    } else {
        " Keybindings  [?] close "
    };

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title);
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    frame.render_widget(Paragraph::new(lines).scroll((effective_scroll, 0)), inner);
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
pub fn status_bar_hints(view: &AppView) -> &'static [(&'static str, &'static str)] {
    match view {
        AppView::TicketList => &[
            ("c", "checkout"),
            ("/", "search"),
            ("f", "filter"),
            ("n", "new ticket"),
            ("r", "refresh"),
            ("?", "help"),
        ],
        AppView::TicketDetail { .. } => &[
            ("t", "status"),
            ("a", "assign self"),
            ("c", "checkout"),
            ("o", "open PR"),
            ("b", "open in browser"),
            ("?", "help"),
        ],
        AppView::TransitionPicker { .. } => &[
            ("type", "filter"),
            ("Enter", "apply"),
            ("?", "help"),
        ],
        AppView::FilterPanel => &[
            ("Space", "toggle"),
            ("Enter", "apply"),
            ("Ctrl+S", "save settings"),
            ("?", "help"),
        ],
        AppView::Settings => &[
            ("Ctrl+S", "save"),
            ("Ctrl+D", "edit settings"),
            ("Ctrl+T", "edit templates"),
            ("?", "help"),
        ],
        AppView::TemplatesPanel => &[
            ("r", "reload templates"),
            ("Ctrl+T", "edit templates.yaml"),
            ("?", "help"),
        ],
        AppView::CreateTicket => &[
            ("Ctrl+S", "submit"),
            ("?", "help"),
        ],
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
