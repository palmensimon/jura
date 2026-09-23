use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tui_textarea::TextArea;

use crate::{
    config::TicketTemplate,
    jira::{
        CreateIssueFields, CreateIssueRequest, NameRef, ProjectRef,
    },
    tui::app::{App, AppEvent, AppView},
};

pub struct CreateState {
    pub template_idx: usize,
    pub summary_input: TextArea<'static>,
    pub description_input: TextArea<'static>,
    pub active_field: usize,
    pub editing: bool,
    pub loading: bool,
}

impl CreateState {
    pub fn new() -> Self {
        let mut summary = TextArea::default();
        summary.set_placeholder_text("Issue summary (required)");
        summary.set_cursor_line_style(Style::default());
        summary.set_block(field_block("Summary", true, false));

        let mut description = TextArea::default();
        description.set_placeholder_text("Description (optional)");
        description.set_cursor_line_style(Style::default());
        description.set_block(field_block("Description", false, false));

        Self {
            template_idx: 0,
            summary_input: summary,
            description_input: description,
            active_field: 0,
            editing: false,
            loading: false,
        }
    }
}

pub fn handle_paste(state: &mut CreateState, text: &str) {
    if state.loading {
        return;
    }
    match state.active_field {
        0 => { state.summary_input.insert_str(text); }
        1 => { state.description_input.insert_str(text); }
        _ => {}
    }
}

pub fn handle_key(app: &mut App, state: &mut CreateState, key: KeyEvent) {
    if state.loading {
        return;
    }

    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.loading = true;
        submit_ticket(app, state);
        return;
    }

    if key.code == KeyCode::Tab || key.code == KeyCode::BackTab {
        state.active_field = (state.active_field + 1) % 2;
        state.editing = false;
        update_field_styles(state);
        return;
    }

    if state.editing {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                state.editing = false;
                update_field_styles(state);
            }
            _ => match state.active_field {
                0 => { state.summary_input.input(key); }
                1 => { state.description_input.input(key); }
                _ => {}
            },
        }
        return;
    }

    match key.code {
        KeyCode::Esc => {
            app.view = AppView::TemplatesPanel;
        }
        KeyCode::Enter => {
            state.editing = true;
            update_field_styles(state);
        }
        _ => {}
    }
}

pub fn update_field_styles(state: &mut CreateState) {
    state.summary_input.set_cursor_line_style(Style::default());
    state.summary_input.set_block(field_block("Summary", state.active_field == 0, state.active_field == 0 && state.editing));
    state.description_input.set_cursor_line_style(Style::default());
    state.description_input.set_block(field_block("Description", state.active_field == 1, state.active_field == 1 && state.editing));
}

fn field_block(title: &str, focused: bool, editing: bool) -> Block<'static> {
    let border_style = if editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title = if focused && !editing { format!(" {title} — Enter to edit ") } else { format!(" {title} ") };
    Block::default().borders(Borders::ALL).title(title).border_style(border_style)
}

fn submit_ticket(app: &mut App, state: &mut CreateState) {
    let Some(template) = app.templates.get(state.template_idx).cloned() else {
        return;
    };

    let summary: String = state.summary_input.lines().join("\n");
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        app.error = Some("Summary is required".to_string());
        return;
    }

    let description_text: String = state.description_input.lines().join("\n");
    let description = if description_text.trim().is_empty() {
        None
    } else {
        Some(description_text)
    };

    let components: Vec<NameRef> = template
        .component
        .as_ref()
        .map(|c| vec![NameRef { name: c.clone() }])
        .unwrap_or_default();

    let assignee = template.assignee.as_ref().map(|a| NameRef { name: a.clone() });
    let team = template.team.clone();
    let fix_versions: Vec<NameRef> = template
        .fix_version
        .as_ref()
        .map(|v| vec![NameRef { name: v.clone() }])
        .unwrap_or_default();

    let req = CreateIssueRequest {
        fields: CreateIssueFields {
            project: ProjectRef { key: template.project.clone() },
            summary,
            issuetype: NameRef { name: template.issue_type.clone() },
            description,
            components,
            labels: template.labels.clone(),
            priority: template.priority.as_ref().map(|p| NameRef { name: p.clone() }),
            epic_link: template.epic_link.clone(),
            assignee,
            team,
            fix_versions,
        },
    };

    let client = app.client.clone();
    let tx = app.event_tx.clone();

    tokio::spawn(async move {
        let result = client.create_issue(&req).await;
        let _ = tx
            .send(match result {
                Ok(resp) => AppEvent::TicketCreated(resp.key),
                Err(e) => AppEvent::Error(format!("{e:#}")),
            })
            .await;
    });
}

pub fn draw(app: &App, state: &mut CreateState, frame: &mut Frame, area: Rect) {
    let template_name = app
        .templates
        .get(state.template_idx)
        .map(|t| t.name.as_str())
        .unwrap_or("New Ticket");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // header
            Constraint::Length(3),  // template info
            Constraint::Length(3),  // summary input
            Constraint::Min(6),     // description
            Constraint::Length(2),  // footer
        ])
        .split(area);

    let header = Paragraph::new(Line::from(vec![
        Span::styled(" Create Ticket", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" — {template_name}"), Style::default().fg(Color::DarkGray)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(header, chunks[0]);

    if let Some(template) = app.templates.get(state.template_idx) {
        draw_template_info(template, frame, chunks[1]);
    }

    draw_text_field(&state.summary_input, state.active_field == 0, state.active_field == 0 && state.editing, "Summary", "Issue summary (required)", frame, chunks[2]);
    draw_text_field(&state.description_input, state.active_field == 1, state.active_field == 1 && state.editing, "Description", "Description (optional)", frame, chunks[3]);

    if state.loading {
        frame.render_widget(
            Paragraph::new(Span::styled(" Creating ticket…", Style::default().fg(Color::Yellow))),
            chunks[4],
        );
    } else if let Some(err) = &app.error {
        frame.render_widget(
            Paragraph::new(Span::styled(format!(" ⚠ {err}"), Style::default().fg(Color::Red))),
            chunks[4],
        );
    }
}

/// `tui_textarea::TextArea` has no line-wrap support (long lines scroll horizontally instead),
/// which is fine while actively editing but reads badly for a field you're not typing into.
/// Only render the raw live text area while `editing`; otherwise show a wrapped read-only
/// preview (bordered Yellow if `focused`, DarkGray otherwise — matching Settings/Template
/// Editor's focused/editing/idle convention).
fn draw_text_field(ta: &TextArea<'static>, focused: bool, editing: bool, title: &str, placeholder: &str, frame: &mut Frame, area: Rect) {
    if editing {
        frame.render_widget(ta, area);
        return;
    }
    let block = field_block(title, focused, false);
    let text = ta.lines().join("\n");
    if text.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(placeholder, Style::default().fg(Color::DarkGray))).block(block),
            area,
        );
    } else {
        frame.render_widget(Paragraph::new(text).block(block).wrap(Wrap { trim: false }), area);
    }
}

fn draw_template_info(template: &TicketTemplate, frame: &mut Frame, area: Rect) {
    let mut parts = vec![
        meta_label("type: "),
        Span::raw(template.issue_type.as_str()),
    ];
    if let Some(c) = &template.component {
        parts.push(meta_label("  component: "));
        parts.push(Span::raw(c.as_str()));
    }
    if let Some(e) = &template.epic_link {
        parts.push(meta_label("  epic: "));
        parts.push(Span::raw(e.as_str()));
    }
    if let Some(a) = &template.assignee {
        parts.push(meta_label("  assignee: "));
        parts.push(Span::raw(a.as_str()));
    }
    if let Some(t) = &template.team {
        parts.push(meta_label("  team: "));
        parts.push(Span::raw(t.as_str()));
    }
    if let Some(fv) = &template.fix_version {
        parts.push(meta_label("  fix version: "));
        parts.push(Span::raw(fv.as_str()));
    }
    frame.render_widget(Paragraph::new(Line::from(parts)), area);
}

fn meta_label(s: &str) -> Span<'_> {
    Span::styled(s, Style::default().fg(Color::DarkGray))
}
