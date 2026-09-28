//! Shared behavior for editable text fields across form screens (Create Ticket, Settings,
//! Template Editor, Filter Panel): Enter starts editing, Esc is the only way out, and while
//! editing Enter inserts a newline in multi-line fields and is ignored in single-line ones.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders},
};
use tui_textarea::TextArea;

pub enum EditOutcome {
    Exit,
    Continue,
}

/// Key handling for a field that is in edit mode.
pub fn handle_editing_key(ta: &mut TextArea<'static>, key: KeyEvent, multiline: bool) -> EditOutcome {
    match key.code {
        KeyCode::Esc => EditOutcome::Exit,
        KeyCode::Enter if !multiline => EditOutcome::Continue,
        _ => {
            ta.input(key);
            EditOutcome::Continue
        }
    }
}

/// Insert pasted text, flattening newlines for single-line fields.
pub fn paste(ta: &mut TextArea<'static>, text: &str, multiline: bool) {
    if multiline {
        ta.insert_str(text);
    } else {
        ta.insert_str(text.replace(['\r', '\n'], " "));
    }
}

/// Green = editing, Yellow = focused, DarkGray = idle; focused fields show an "Enter to edit" hint.
pub fn field_block(label: &str, focused: bool, editing: bool) -> Block<'static> {
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if focused && !editing { format!(" {label} — Enter to edit ") } else { format!(" {label} ") };
    Block::default().borders(Borders::ALL).title(title).border_style(border_style)
}

pub fn update_field_block(ta: &mut TextArea<'static>, label: &str, focused: bool, editing: bool) {
    ta.set_block(field_block(label, focused, editing));
    if editing {
        ta.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    } else {
        ta.set_cursor_style(Style::default());
    }
}

pub fn single_line_area(value: &str) -> TextArea<'static> {
    let mut ta = TextArea::from([value]);
    ta.move_cursor(tui_textarea::CursorMove::End);
    ta
}
