use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use tui_textarea::TextArea;

use super::search_picker::{self, SearchPickerAction, SearchPickerState};
use crate::{
    config::{TicketTemplate, save_templates},
    tui::app::{App, AppEvent, AppView},
};

// ── Field enums ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemplateEditorField {
    Name,
    Project,
    IssueType,
    Component,
    EpicLink,
    Team,
    Priority,
    FixVersion,
    Assignee,
    Labels,
}

const FIELD_COUNT: usize = 10;

impl TemplateEditorField {
    fn index(self) -> usize {
        match self {
            Self::Name => 0,
            Self::Project => 1,
            Self::IssueType => 2,
            Self::Component => 3,
            Self::EpicLink => 4,
            Self::Team => 5,
            Self::Priority => 6,
            Self::FixVersion => 7,
            Self::Assignee => 8,
            Self::Labels => 9,
        }
    }

    fn from_index(i: usize) -> Self {
        match i % FIELD_COUNT {
            0 => Self::Name,
            1 => Self::Project,
            2 => Self::IssueType,
            3 => Self::Component,
            4 => Self::EpicLink,
            5 => Self::Team,
            6 => Self::Priority,
            7 => Self::FixVersion,
            8 => Self::Assignee,
            _ => Self::Labels,
        }
    }

    fn is_text(self) -> bool {
        matches!(self, Self::Name | Self::Project | Self::Assignee | Self::Labels)
    }

    fn picker_field(self) -> Option<TemplateField> {
        match self {
            Self::IssueType => Some(TemplateField::IssueType),
            Self::Component => Some(TemplateField::Component),
            Self::EpicLink => Some(TemplateField::EpicLink),
            Self::Team => Some(TemplateField::Team),
            Self::Priority => Some(TemplateField::Priority),
            Self::FixVersion => Some(TemplateField::FixVersion),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Name => "[1] Name",
            Self::Project => "[2] Project",
            Self::IssueType => "[3] Issue Type",
            Self::Component => "[4] Component",
            Self::EpicLink => "[5] Epic Link",
            Self::Team => "[6] Team",
            Self::Priority => "[7] Priority",
            Self::FixVersion => "[8] Fix Version",
            Self::Assignee => "[9] Assignee",
            Self::Labels => "[0] Labels (comma-separated)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TemplateField {
    IssueType,
    Component,
    EpicLink,
    Team,
    Priority,
    FixVersion,
}

impl TemplateField {
    /// Live fields re-query the server on every keystroke; the rest fetch once and filter locally.
    fn is_live(self) -> bool {
        matches!(self, Self::EpicLink | Self::Team)
    }

    fn requires_project(self) -> bool {
        matches!(self, Self::IssueType | Self::Component | Self::EpicLink | Self::FixVersion)
    }

    fn is_required(self) -> bool {
        matches!(self, Self::IssueType)
    }

    fn label(self) -> &'static str {
        match self {
            Self::IssueType => "Issue Type",
            Self::Component => "Component",
            Self::EpicLink => "Epic Link",
            Self::Team => "Team",
            Self::Priority => "Priority",
            Self::FixVersion => "Fix Version",
        }
    }
}

fn has_clear_row(field: TemplateField) -> bool {
    !field.is_required()
}

// ── State ─────────────────────────────────────────────────────────────────────

pub struct FieldPickerState {
    pub field: TemplateField,
    pub picker: SearchPickerState,
}

pub struct TemplateEditorState {
    pub index: Option<usize>,
    pub draft: TicketTemplate,
    name_input: TextArea<'static>,
    project_input: TextArea<'static>,
    assignee_input: TextArea<'static>,
    labels_input: TextArea<'static>,
    pub active: TemplateEditorField,
    editing_text: bool,
    pub field_picker: Option<FieldPickerState>,
    pub saving: bool,
}

fn empty_template(app: &App) -> TicketTemplate {
    TicketTemplate {
        name: String::new(),
        project: app.config.project.clone().unwrap_or_default(),
        issue_type: String::new(),
        component: None,
        epic_link: None,
        labels: vec![],
        priority: None,
        team: None,
        assignee: None,
        fix_version: None,
    }
}

impl TemplateEditorState {
    pub fn new(app: &App, index: Option<usize>) -> Self {
        let draft = match index {
            Some(i) => app.templates.get(i).cloned().unwrap_or_else(|| empty_template(app)),
            None => empty_template(app),
        };
        let name_input = single_line_area(&draft.name);
        let project_input = single_line_area(&draft.project);
        let assignee_input = single_line_area(draft.assignee.as_deref().unwrap_or(""));
        let labels_input = single_line_area(&draft.labels.join(", "));

        let mut state = Self {
            index,
            draft,
            name_input,
            project_input,
            assignee_input,
            labels_input,
            active: TemplateEditorField::Name,
            editing_text: false,
            field_picker: None,
            saving: false,
        };
        state.refresh_styles();
        state
    }

    /// True while a keystroke would go into a text field or a field-picker's search box,
    /// rather than being interpreted as a navigation/action shortcut — used to suppress the
    /// global `?`-help-toggle so a literal `?` can still be typed.
    pub fn is_capturing_text(&self) -> bool {
        self.editing_text || self.field_picker.is_some()
    }

    fn text_area_mut(&mut self, field: TemplateEditorField) -> &mut TextArea<'static> {
        match field {
            TemplateEditorField::Name => &mut self.name_input,
            TemplateEditorField::Project => &mut self.project_input,
            TemplateEditorField::Assignee => &mut self.assignee_input,
            TemplateEditorField::Labels => &mut self.labels_input,
            _ => unreachable!("text_area_mut called with a picker-backed field"),
        }
    }

    fn refresh_styles(&mut self) {
        let specs = [
            (TemplateEditorField::Name, "Name"),
            (TemplateEditorField::Project, "Project"),
            (TemplateEditorField::Assignee, "Assignee"),
            (TemplateEditorField::Labels, "Labels (comma-separated)"),
        ];
        for (field, label) in specs {
            let focused = self.active == field;
            let editing = focused && self.editing_text;
            update_field_block(self.text_area_mut(field), label, focused, editing);
        }
    }
}

// ── Key handling ─────────────────────────────────────────────────────────────

pub fn handle_key(app: &mut App, state: &mut TemplateEditorState, key: KeyEvent) {
    if state.field_picker.is_some() {
        handle_field_picker_key(app, state, key);
        return;
    }

    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if !state.saving {
            save(app, state);
        }
        return;
    }

    if state.editing_text {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                state.editing_text = false;
                state.refresh_styles();
            }
            _ => {
                state.text_area_mut(state.active).input(key);
            }
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.view = AppView::TemplatesPanel;
        }
        KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
            state.active = TemplateEditorField::from_index(state.active.index() + 1);
            state.refresh_styles();
        }
        KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
            state.active = TemplateEditorField::from_index(state.active.index() + FIELD_COUNT - 1);
            state.refresh_styles();
        }
        KeyCode::Char(c @ '1'..='9') => {
            state.active = TemplateEditorField::from_index(c as usize - '1' as usize);
            state.refresh_styles();
        }
        KeyCode::Char('0') => {
            state.active = TemplateEditorField::from_index(9);
            state.refresh_styles();
        }
        KeyCode::Enter => {
            if state.active.is_text() {
                state.editing_text = true;
                state.refresh_styles();
            } else if let Some(field) = state.active.picker_field() {
                open_picker(app, state, field);
            }
        }
        _ => {}
    }
}

fn open_picker(app: &mut App, state: &mut TemplateEditorState, field: TemplateField) {
    if field.requires_project() && state.draft.project.trim().is_empty() {
        app.error = Some("Set a project first".to_string());
        return;
    }
    app.error = None;
    state.field_picker = Some(FieldPickerState { field, picker: SearchPickerState::new() });

    if field == TemplateField::EpicLink {
        // Don't hit the server for an unscoped epic search — wait for at least one character.
        app.field_picker_items.clear();
        app.field_picker_loading = false;
        return;
    }
    spawn_query(app, field, state.draft.project.clone(), String::new());
}

fn handle_field_picker_key(app: &mut App, state: &mut TemplateEditorState, key: KeyEvent) {
    let Some(fp) = state.field_picker.as_mut() else { return };
    let field = fp.field;
    let live = field.is_live();
    let show_clear_row = has_clear_row(field);

    let action = search_picker::handle_key(&mut fp.picker, key, &app.field_picker_items, live, show_clear_row);

    match action {
        SearchPickerAction::None => {}
        SearchPickerAction::Cancel => {
            state.field_picker = None;
        }
        SearchPickerAction::Requery => {
            if live {
                let query = state.field_picker.as_ref().unwrap().picker.search.clone();
                if field == TemplateField::EpicLink && query.is_empty() {
                    app.field_picker_items.clear();
                    app.field_picker_loading = false;
                } else {
                    spawn_query(app, field, state.draft.project.clone(), query);
                }
            }
        }
        SearchPickerAction::Selected(value) => {
            apply_field_value(state, field, value);
            state.field_picker = None;
        }
    }
}

fn apply_field_value(state: &mut TemplateEditorState, field: TemplateField, value: Option<String>) {
    match field {
        TemplateField::IssueType => state.draft.issue_type = value.unwrap_or_default(),
        TemplateField::Component => state.draft.component = value,
        TemplateField::EpicLink => state.draft.epic_link = value,
        TemplateField::Team => state.draft.team = value,
        TemplateField::Priority => state.draft.priority = value,
        TemplateField::FixVersion => state.draft.fix_version = value,
    }
}

fn spawn_query(app: &mut App, field: TemplateField, project: String, query: String) {
    app.field_picker_seq += 1;
    let seq = app.field_picker_seq;
    app.field_picker_items.clear();
    app.field_picker_loading = true;

    let client = app.client.clone();
    let tx = app.event_tx.clone();

    tokio::spawn(async move {
        let result: Result<Vec<(String, String)>, String> = match field {
            TemplateField::Component => client
                .get_project_components(&project)
                .await
                .map(|cs| cs.into_iter().map(|c| (c.name.clone(), c.name)).collect())
                .map_err(|e| format!("{e:#}")),
            TemplateField::IssueType => client
                .get_issue_types(&project)
                .await
                .map(|its| its.into_iter().map(|t| (t.name.clone(), t.name)).collect())
                .map_err(|e| format!("{e:#}")),
            TemplateField::FixVersion => client
                .get_project_versions(&project)
                .await
                .map(|vs| vs.into_iter().map(|v| (v.name.clone(), v.name)).collect())
                .map_err(|e| format!("{e:#}")),
            TemplateField::Priority => client
                .get_priorities()
                .await
                .map(|ps| ps.into_iter().map(|p| (p.name.clone(), p.name)).collect())
                .map_err(|e| format!("{e:#}")),
            TemplateField::Team => client
                .search_field_suggestions("Team", &query)
                .await
                .map(|ss| ss.into_iter().map(|s| (s.value, s.display_name)).collect())
                .map_err(|e| format!("{e:#}")),
            TemplateField::EpicLink => client
                .search_epics(&project, &query, 25)
                .await
                .map_err(|e| format!("{e:#}")),
        };
        let _ = tx.send(AppEvent::FieldSuggestionsLoaded(seq, result)).await;
    });
}

fn save(app: &mut App, state: &mut TemplateEditorState) {
    let name = first_line(&state.name_input);
    let project = first_line(&state.project_input);
    let assignee_raw = first_line(&state.assignee_input);
    let labels_raw = first_line(&state.labels_input);

    if name.is_empty() || project.is_empty() || state.draft.issue_type.trim().is_empty() {
        app.error = Some("Name, project and issue type are required".to_string());
        return;
    }

    let mut draft = state.draft.clone();
    draft.name = name;
    draft.project = project;
    draft.assignee = if assignee_raw.is_empty() { None } else { Some(assignee_raw) };
    draft.labels = labels_raw
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut new_list = app.templates.clone();
    match state.index {
        Some(i) if i < new_list.len() => new_list[i] = draft,
        _ => new_list.push(draft),
    }

    state.saving = true;
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

fn first_line(ta: &TextArea<'static>) -> String {
    ta.lines().first().cloned().unwrap_or_default().trim().to_string()
}

// ── Drawing ───────────────────────────────────────────────────────────────────

pub fn draw(app: &App, state: &mut TemplateEditorState, frame: &mut Frame, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Length(3), // name
            Constraint::Length(3), // project
            Constraint::Length(1), // issue type
            Constraint::Length(1), // component
            Constraint::Length(1), // epic link
            Constraint::Length(1), // team
            Constraint::Length(1), // priority
            Constraint::Length(1), // fix version
            Constraint::Length(3), // assignee
            Constraint::Length(3), // labels
            Constraint::Length(2), // footer
        ])
        .split(area);

    let title = match state.index {
        Some(_) if !state.draft.name.is_empty() => format!(" Edit Template — {} ", state.draft.name),
        Some(_) => " Edit Template ".to_string(),
        None => " New Template ".to_string(),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )))
        .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray))),
        chunks[0],
    );

    frame.render_widget(&state.name_input, chunks[1]);
    frame.render_widget(&state.project_input, chunks[2]);

    draw_value_row(
        frame,
        chunks[3],
        TemplateEditorField::IssueType,
        state.active,
        if state.draft.issue_type.is_empty() { "(required)".to_string() } else { state.draft.issue_type.clone() },
    );
    draw_value_row(
        frame,
        chunks[4],
        TemplateEditorField::Component,
        state.active,
        state.draft.component.clone().unwrap_or_else(|| "—".to_string()),
    );
    draw_value_row(
        frame,
        chunks[5],
        TemplateEditorField::EpicLink,
        state.active,
        state.draft.epic_link.clone().unwrap_or_else(|| "—".to_string()),
    );
    draw_value_row(
        frame,
        chunks[6],
        TemplateEditorField::Team,
        state.active,
        state.draft.team.clone().unwrap_or_else(|| "—".to_string()),
    );
    draw_value_row(
        frame,
        chunks[7],
        TemplateEditorField::Priority,
        state.active,
        state.draft.priority.clone().unwrap_or_else(|| "—".to_string()),
    );
    draw_value_row(
        frame,
        chunks[8],
        TemplateEditorField::FixVersion,
        state.active,
        state.draft.fix_version.clone().unwrap_or_else(|| "—".to_string()),
    );

    frame.render_widget(&state.assignee_input, chunks[9]);
    frame.render_widget(&state.labels_input, chunks[10]);

    let footer = if let Some(err) = &app.error {
        Line::from(Span::styled(format!(" ⚠  {err}"), Style::default().fg(Color::Red)))
    } else if state.saving {
        Line::from(Span::styled(" Saving…", Style::default().fg(Color::Yellow)))
    } else {
        Line::from(Span::styled(" Ctrl+S save   Esc cancel", Style::default().fg(Color::DarkGray)))
    };
    frame.render_widget(
        Paragraph::new(footer).block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray))),
        chunks[11],
    );

    if let Some(picker) = &state.field_picker {
        draw_field_picker(app, picker, frame, area);
    }
}

fn draw_value_row(frame: &mut Frame, area: Rect, field: TemplateEditorField, active: TemplateEditorField, value: String) {
    let is_active = active == field;
    let label_style = if is_active {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let value_style = if is_active { Style::default().fg(Color::White) } else { Style::default().fg(Color::Gray) };
    let hint = if is_active { "  (Enter to change)" } else { "" };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{}: ", field.label()), label_style),
            Span::styled(value, value_style),
            Span::styled(hint, Style::default().fg(Color::DarkGray)),
        ])),
        area,
    );
}

fn draw_field_picker(app: &App, fp: &FieldPickerState, frame: &mut Frame, area: Rect) {
    let awaiting_epic_input = fp.field == TemplateField::EpicLink && fp.picker.search.is_empty();
    let opts = search_picker::DrawOptions {
        title: fp.field.label(),
        live: fp.field.is_live(),
        show_clear_row: has_clear_row(fp.field),
        loading: app.field_picker_loading,
        awaiting_input: awaiting_epic_input.then_some("Type at least 1 character to search epics…"),
    };
    search_picker::draw(&app.field_picker_items, &fp.picker, &opts, frame, area);
}

fn single_line_area(value: &str) -> TextArea<'static> {
    let mut ta = TextArea::from([value]);
    ta.move_cursor(tui_textarea::CursorMove::End);
    ta
}

fn update_field_block(ta: &mut TextArea<'static>, label: &str, focused: bool, editing: bool) {
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if focused && !editing { format!(" {label} — Enter to edit ") } else { format!(" {label} ") };
    ta.set_block(Block::default().borders(Borders::ALL).title(title).border_style(border_style));
    if editing {
        ta.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    } else {
        ta.set_cursor_style(Style::default());
    }
}
