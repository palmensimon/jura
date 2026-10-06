use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

use crate::tui::app::{ConfirmAction, PendingConfirm};

/// Renders the "this ticket belongs to someone else" confirmation popup.
pub fn draw(pending: &PendingConfirm, frame: &mut Frame, area: Rect) {
    let (issue, question) = match &pending.action {
        ConfirmAction::Transition { issue, .. } => (issue, "Change its status anyway?"),
        ConfirmAction::ToggleAssign { issue } => (issue, "Assign it to yourself anyway?"),
        ConfirmAction::Checkout { issue } => (issue, "Checking out will reassign it to you. Continue?"),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(issue.key.as_str(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" is assigned to "),
            Span::styled(pending.assignee.as_str(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("."),
        ]),
        Line::from(question),
        Line::from(""),
        Line::from(Span::styled("[y/↵] continue   [n/Esc] cancel", Style::default().fg(Color::DarkGray))),
    ];

    let popup_w = area.width.saturating_sub(4).min(64);
    let popup_h = (lines.len() as u16 + 2).min(area.height);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::default().fg(Color::Yellow));
    frame.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), popup);
}
