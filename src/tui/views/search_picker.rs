use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
};

/// Generic "type to filter/search a `(value, label)` list, Enter selects, Esc cancels" popup.
/// Shared by any view that needs a searchable picker backed by live or static data — the caller
/// owns what's being searched (which API to call, project-scoping, whether a field is required)
/// and where the item list itself lives; this only owns the search text/cursor and rendering.
pub struct SearchPickerState {
    pub search: String,
    pub selected: usize,
}

impl SearchPickerState {
    pub fn new() -> Self {
        Self { search: String::new(), selected: 0 }
    }
}

/// What happened as a result of a keypress — the caller applies the actual effect.
pub enum SearchPickerAction {
    /// State already mutated (navigation); nothing further to do.
    None,
    /// The search text changed — caller decides whether/how to re-query (only relevant when
    /// `live`; a static picker's `draw` call will just re-filter using the new text).
    Requery,
    /// The user picked a value, or cleared the field via the pinned "— Clear —" row (`None`).
    Selected(Option<String>),
    /// Esc, or Backspace on an already-empty search — caller should close the picker.
    Cancel,
}

/// Items to actually render: already server-filtered if `live`, else filtered locally by `search`.
pub fn visible_items<'a>(items: &'a [(String, String)], search: &str, live: bool) -> Vec<&'a (String, String)> {
    if live || search.is_empty() {
        return items.iter().collect();
    }
    let q = search.to_lowercase();
    items.iter().filter(|(_, label)| label.to_lowercase().contains(&q)).collect()
}

pub fn handle_key(
    state: &mut SearchPickerState,
    key: KeyEvent,
    items: &[(String, String)],
    live: bool,
    show_clear_row: bool,
) -> SearchPickerAction {
    match key.code {
        KeyCode::Esc => SearchPickerAction::Cancel,
        KeyCode::Backspace => {
            if state.search.is_empty() {
                SearchPickerAction::Cancel
            } else {
                state.search.pop();
                state.selected = 0;
                SearchPickerAction::Requery
            }
        }
        KeyCode::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
            SearchPickerAction::None
        }
        KeyCode::Down => {
            let offset = if show_clear_row { 1 } else { 0 };
            let total = offset + visible_items(items, &state.search, live).len();
            if state.selected + 1 < total {
                state.selected += 1;
            }
            SearchPickerAction::None
        }
        KeyCode::Char(c) => {
            state.search.push(c);
            state.selected = 0;
            SearchPickerAction::Requery
        }
        KeyCode::Enter => {
            let offset = if show_clear_row { 1 } else { 0 };
            let visible = visible_items(items, &state.search, live);
            let total = offset + visible.len();
            if total == 0 {
                SearchPickerAction::None
            } else {
                let idx = state.selected.min(total - 1);
                if show_clear_row && idx == 0 {
                    SearchPickerAction::Selected(None)
                } else {
                    SearchPickerAction::Selected(Some(visible[idx - offset].0.clone()))
                }
            }
        }
        _ => SearchPickerAction::None,
    }
}

/// Rendering options for `draw` — kept as one struct since they're always specified together.
pub struct DrawOptions<'a> {
    pub title: &'a str,
    pub live: bool,
    pub show_clear_row: bool,
    pub loading: bool,
    /// When set, shown instead of the list (e.g. "type at least 1 character…" gating on an
    /// expensive live search) — the caller decides when this applies.
    pub awaiting_input: Option<&'a str>,
}

pub fn draw(items: &[(String, String)], state: &SearchPickerState, opts: &DrawOptions, frame: &mut Frame, area: Rect) {
    let popup_w = (area.width * 60 / 100).max(50).min(area.width);
    let visible = visible_items(items, &state.search, opts.live);
    let offset = if opts.show_clear_row { 1 } else { 0 };
    let list_rows: u16 = if opts.loading || opts.awaiting_input.is_some() {
        1
    } else {
        (offset + visible.len()).max(1).min(14) as u16
    };
    let popup_h = (list_rows + 5).min(area.height.saturating_sub(4)).max(8);
    let x = area.x + area.width.saturating_sub(popup_w) / 2;
    let y = area.y + area.height.saturating_sub(popup_h) / 2;
    let popup = Rect::new(x, y, popup_w, popup_h);

    frame.render_widget(Clear, popup);
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", opts.title))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let footer = Paragraph::new(Span::styled(" [↵] select   [Esc] cancel", Style::default().fg(Color::DarkGray)));

    let placeholder = if opts.live { "type to search…" } else { "type to filter…" };
    let search_line = if state.search.is_empty() {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {placeholder}"), Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(state.search.clone(), Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Yellow)),
        ])
    };
    frame.render_widget(
        Paragraph::new(search_line).block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray))),
        chunks[0],
    );

    if let Some(msg) = opts.awaiting_input {
        frame.render_widget(Paragraph::new(Span::styled(format!(" {msg}"), Style::default().fg(Color::DarkGray))), chunks[1]);
        frame.render_widget(footer, chunks[2]);
        return;
    }

    if opts.loading {
        frame.render_widget(Paragraph::new(Span::styled(" Loading…", Style::default().fg(Color::DarkGray))), chunks[1]);
        frame.render_widget(footer, chunks[2]);
        return;
    }

    let mut list_items: Vec<ListItem> = vec![];
    if opts.show_clear_row {
        list_items.push(ListItem::new(Line::from(Span::styled("— Clear —", Style::default().fg(Color::DarkGray)))));
    }
    list_items.extend(
        visible
            .iter()
            .map(|(_, label)| ListItem::new(Line::from(Span::styled(label.as_str(), Style::default().fg(Color::White))))),
    );

    if list_items.is_empty() {
        frame.render_widget(Paragraph::new(Span::styled(" No matches", Style::default().fg(Color::DarkGray))), chunks[1]);
        frame.render_widget(footer, chunks[2]);
        return;
    }

    let total = list_items.len();
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::NONE))
        .highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let selected = state.selected.min(total.saturating_sub(1));
    let mut list_state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, chunks[1], &mut list_state);
    frame.render_widget(footer, chunks[2]);
}
