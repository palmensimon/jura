use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use super::search_picker::{self, SearchPickerAction, SearchPickerState};
use crate::{
    config::{EpicEntry, TeamEntry, save_settings},
    tui::app::{App, AppEvent},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionCategory {
    Statuses,
    HiddenStatuses,
    Components,
    Labels,
    Teams,
    Epics,
}

const CATEGORY_COUNT: usize = 6;

impl OptionCategory {
    fn index(self) -> usize {
        match self {
            Self::Statuses => 0,
            Self::HiddenStatuses => 1,
            Self::Components => 2,
            Self::Labels => 3,
            Self::Teams => 4,
            Self::Epics => 5,
        }
    }

    fn from_index(i: usize) -> Self {
        match i % CATEGORY_COUNT {
            0 => Self::Statuses,
            1 => Self::HiddenStatuses,
            2 => Self::Components,
            3 => Self::Labels,
            4 => Self::Teams,
            _ => Self::Epics,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Statuses => "[1] Statuses",
            Self::HiddenStatuses => "[2] Hidden Statuses",
            Self::Components => "[3] Components",
            Self::Labels => "[4] Labels",
            Self::Teams => "[5] Teams",
            Self::Epics => "[6] Epics",
        }
    }

    /// Live fields re-query the server on every keystroke; the rest fetch once and filter locally.
    fn is_live(self) -> bool {
        matches!(self, Self::Labels | Self::Teams | Self::Epics)
    }

    fn requires_project(self) -> bool {
        matches!(self, Self::Components | Self::Epics)
    }
}

pub struct FilterOptionsState {
    statuses: Vec<String>,
    hidden_statuses: Vec<String>,
    components: Vec<String>,
    labels: Vec<String>,
    teams: Vec<TeamEntry>,
    epics: Vec<EpicEntry>,
    active: OptionCategory,
    selected_idx: usize,
    picker: Option<SearchPickerState>,
    pub saving: bool,
}

impl FilterOptionsState {
    pub fn new(app: &App) -> Self {
        let d = &app.config.defaults;
        Self {
            statuses: d.visible_statuses.clone(),
            hidden_statuses: d.hidden_statuses.clone(),
            components: d.visible_components.clone(),
            labels: d.visible_labels.clone(),
            teams: d.visible_teams.clone(),
            epics: d.visible_epics.clone(),
            active: OptionCategory::Statuses,
            selected_idx: 0,
            picker: None,
            saving: false,
        }
    }

    pub fn is_capturing_text(&self) -> bool {
        self.picker.is_some()
    }

    fn current_items(&self) -> Vec<(String, String)> {
        match self.active {
            OptionCategory::Statuses => self.statuses.iter().map(|s| (s.clone(), s.clone())).collect(),
            OptionCategory::HiddenStatuses => self.hidden_statuses.iter().map(|s| (s.clone(), s.clone())).collect(),
            OptionCategory::Components => self.components.iter().map(|s| (s.clone(), s.clone())).collect(),
            OptionCategory::Labels => self.labels.iter().map(|s| (s.clone(), s.clone())).collect(),
            OptionCategory::Teams => self.teams.iter().map(|t| (t.id.clone(), t.name.clone())).collect(),
            OptionCategory::Epics => self.epics.iter().map(|e| (e.id.clone(), e.name.clone())).collect(),
        }
    }

    fn current_ids(&self) -> Vec<String> {
        self.current_items().into_iter().map(|(v, _)| v).collect()
    }

    fn remove_selected(&mut self) {
        let idx = self.selected_idx;
        match self.active {
            OptionCategory::Statuses => remove_at(&mut self.statuses, idx),
            OptionCategory::HiddenStatuses => remove_at(&mut self.hidden_statuses, idx),
            OptionCategory::Components => remove_at(&mut self.components, idx),
            OptionCategory::Labels => remove_at(&mut self.labels, idx),
            OptionCategory::Teams => remove_at(&mut self.teams, idx),
            OptionCategory::Epics => remove_at(&mut self.epics, idx),
        }
        let len = self.current_items().len();
        if self.selected_idx >= len {
            self.selected_idx = len.saturating_sub(1);
        }
    }

    fn add(&mut self, value: String, label: String) {
        match self.active {
            OptionCategory::Statuses => push_unique(&mut self.statuses, value),
            OptionCategory::HiddenStatuses => push_unique(&mut self.hidden_statuses, value),
            OptionCategory::Components => push_unique(&mut self.components, value),
            OptionCategory::Labels => push_unique(&mut self.labels, value),
            OptionCategory::Teams => {
                if !self.teams.iter().any(|t| t.id == value) {
                    self.teams.push(TeamEntry { id: value, name: label });
                }
            }
            OptionCategory::Epics => {
                if !self.epics.iter().any(|e| e.id == value) {
                    self.epics.push(EpicEntry { id: value, name: label });
                }
            }
        }
    }

}

fn remove_at<T>(v: &mut Vec<T>, idx: usize) {
    if idx < v.len() {
        v.remove(idx);
    }
}

fn push_unique(v: &mut Vec<String>, value: String) {
    if !v.contains(&value) {
        v.push(value);
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut FilterOptionsState, key: KeyEvent) {
    if state.picker.is_some() {
        handle_picker_key(app, state, key);
        return;
    }

    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if !state.saving {
            save(app, state);
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.view = crate::tui::app::AppView::FilterPanel;
        }
        KeyCode::Tab | KeyCode::Right => {
            state.active = OptionCategory::from_index(state.active.index() + 1);
            state.selected_idx = 0;
        }
        KeyCode::BackTab | KeyCode::Left => {
            state.active = OptionCategory::from_index(state.active.index() + CATEGORY_COUNT - 1);
            state.selected_idx = 0;
        }
        KeyCode::Char(c @ '1'..='6') => {
            state.active = OptionCategory::from_index(c as usize - '1' as usize);
            state.selected_idx = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.selected_idx > 0 {
                state.selected_idx -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = state.current_items().len();
            if state.selected_idx + 1 < len {
                state.selected_idx += 1;
            }
        }
        KeyCode::Char('x') | KeyCode::Delete => {
            if !state.current_items().is_empty() {
                state.remove_selected();
            }
        }
        KeyCode::Char('a') => {
            open_picker(app, state);
        }
        _ => {}
    }
}

fn open_picker(app: &mut App, state: &mut FilterOptionsState) {
    let category = state.active;
    if category.requires_project() {
        let project = app.filter.project.clone().or_else(|| app.config.project.clone());
        if project.is_none() {
            app.error = Some("Set a project first (in the Filter Panel)".to_string());
            return;
        }
    }
    app.error = None;
    state.picker = Some(SearchPickerState::new());
    spawn_query(app, state, String::new());
}

fn handle_picker_key(app: &mut App, state: &mut FilterOptionsState, key: KeyEvent) {
    let Some(picker) = state.picker.as_mut() else { return };
    let category = state.active;
    let live = category.is_live();

    let action = search_picker::handle_key(picker, key, &app.field_picker_items, live, false);

    match action {
        SearchPickerAction::None => {}
        SearchPickerAction::Cancel => {
            state.picker = None;
        }
        SearchPickerAction::Requery => {
            let query = state.picker.as_ref().unwrap().search.clone();
            spawn_query(app, state, query);
        }
        SearchPickerAction::Selected(Some(value)) => {
            let label = app
                .field_picker_items
                .iter()
                .find(|(v, _)| v == &value)
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| value.clone());
            state.add(value, label);
            state.picker = None;
        }
        SearchPickerAction::Selected(None) => {
            state.picker = None;
        }
    }
}

fn spawn_query(app: &mut App, state: &FilterOptionsState, query: String) {
    let category = state.active;
    let already_present = state.current_ids();
    let project = app.filter.project.clone().or_else(|| app.config.project.clone()).unwrap_or_default();

    app.field_picker_seq += 1;
    let seq = app.field_picker_seq;
    app.field_picker_items.clear();
    app.field_picker_loading = true;

    let client = app.client.clone();
    let tx = app.event_tx.clone();

    tokio::spawn(async move {
        let result: Result<Vec<(String, String)>, String> = match category {
            OptionCategory::Statuses | OptionCategory::HiddenStatuses => client
                .get_statuses()
                .await
                .map(|ss| ss.into_iter().map(|s| (s.clone(), s)).collect())
                .map_err(|e| format!("{e:#}")),
            OptionCategory::Components => client
                .get_project_components(&project)
                .await
                .map(|cs| cs.into_iter().map(|c| (c.name.clone(), c.name)).collect())
                .map_err(|e| format!("{e:#}")),
            OptionCategory::Labels => client
                .search_field_suggestions("labels", &query)
                .await
                .map(|ss| ss.into_iter().map(|s| (s.value, s.display_name)).collect())
                .map_err(|e| format!("{e:#}")),
            OptionCategory::Teams => client
                .search_field_suggestions("Team", &query)
                .await
                .map(|ss| ss.into_iter().map(|s| (s.value, s.display_name)).collect())
                .map_err(|e| format!("{e:#}")),
            OptionCategory::Epics => client
                .search_epics(&project, &query, 25)
                .await
                .map_err(|e| format!("{e:#}")),
        };
        let result = result.map(|items| {
            items.into_iter().filter(|(v, _)| !already_present.contains(v)).collect()
        });
        let _ = tx.send(AppEvent::FieldSuggestionsLoaded(seq, result)).await;
    });
}

fn save(app: &mut App, state: &mut FilterOptionsState) {
    state.saving = true;
    let mut new_cfg = (*app.config).clone();
    new_cfg.defaults.visible_statuses = state.statuses.clone();
    new_cfg.defaults.hidden_statuses = state.hidden_statuses.clone();
    new_cfg.defaults.visible_components = state.components.clone();
    new_cfg.defaults.visible_labels = state.labels.clone();
    new_cfg.defaults.visible_teams = state.teams.clone();
    new_cfg.defaults.visible_epics = state.epics.clone();

    let tx = app.event_tx.clone();
    tokio::spawn(async move {
        match save_settings(&new_cfg.defaults) {
            Ok(()) => {
                let _ = tx.send(AppEvent::FilterOptionsSaved(new_cfg)).await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await;
            }
        }
    });
}

// ── Drawing ───────────────────────────────────────────────────────────────────

pub fn draw(app: &App, state: &FilterOptionsState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Length(2), // category tabs
            Constraint::Min(0),    // items list
            Constraint::Length(2), // footer
        ])
        .split(area);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " Filter Options",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray))),
        chunks[0],
    );

    let categories = [
        OptionCategory::Statuses,
        OptionCategory::HiddenStatuses,
        OptionCategory::Components,
        OptionCategory::Labels,
        OptionCategory::Teams,
        OptionCategory::Epics,
    ];
    let mut tab_spans = vec![Span::raw(" ")];
    for (i, cat) in categories.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::raw("  │  "));
        }
        let count = category_len(state, *cat);
        let style = if *cat == state.active {
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        tab_spans.push(Span::styled(format!("{} ({count})", cat.label()), style));
    }
    frame.render_widget(Paragraph::new(Line::from(tab_spans)), chunks[1]);

    let items = state.current_items();
    if items.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(" (none — press a to add)", Style::default().fg(Color::DarkGray))),
            chunks[2],
        );
    } else {
        let list_items: Vec<ListItem> = items
            .iter()
            .map(|(_, label)| ListItem::new(Line::from(Span::styled(label.as_str(), Style::default().fg(Color::White)))))
            .collect();
        let list = List::new(list_items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");
        let selected = state.selected_idx.min(items.len().saturating_sub(1));
        let mut list_state = ListState::default().with_selected(Some(selected));
        frame.render_stateful_widget(list, chunks[2], &mut list_state);
    }

    let footer = if let Some(err) = &app.error {
        Line::from(Span::styled(format!(" ⚠  {err}"), Style::default().fg(Color::Red)))
    } else if state.saving {
        Line::from(Span::styled(" Saving…", Style::default().fg(Color::Yellow)))
    } else {
        Line::from(Span::styled(
            " Tab switch  [a] add  [x] remove  [Ctrl+S] save  [Esc] cancel",
            Style::default().fg(Color::DarkGray),
        ))
    };
    frame.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray))),
        chunks[3],
    );

    if state.picker.is_some() {
        draw_picker(app, state, frame, area);
    }
}

fn category_len(state: &FilterOptionsState, cat: OptionCategory) -> usize {
    match cat {
        OptionCategory::Statuses => state.statuses.len(),
        OptionCategory::HiddenStatuses => state.hidden_statuses.len(),
        OptionCategory::Components => state.components.len(),
        OptionCategory::Labels => state.labels.len(),
        OptionCategory::Teams => state.teams.len(),
        OptionCategory::Epics => state.epics.len(),
    }
}

fn draw_picker(app: &App, state: &FilterOptionsState, frame: &mut Frame, area: Rect) {
    let Some(picker) = &state.picker else { return };
    let category = state.active;
    let opts = search_picker::DrawOptions {
        title: category.label().trim_start_matches(|c: char| c == '[' || c.is_ascii_digit() || c == ']' || c == ' '),
        live: category.is_live(),
        show_clear_row: false,
        loading: app.field_picker_loading,
        awaiting_input: None,
    };
    search_picker::draw(&app.field_picker_items, picker, &opts, frame, area);
}
