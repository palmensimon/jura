use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
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
    pub loading: bool,
}

impl CreateState {
    pub fn new() -> Self {
        let mut summary = TextArea::default();
        summary.set_placeholder_text("Issue summary (required)");
        summary.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Summary ")
                .border_style(Style::default().fg(Color::Yellow)),
        );

        let mut description = TextArea::default();
        description.set_placeholder_text("Description (optional)");
        description.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Description ")
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        Self {
            template_idx: 0,
            summary_input: summary,
            description_input: description,
            active_field: 0,
            loading: false,
        }
    }
}

pub fn handle_key(app: &mut App, state: &mut CreateState, key: KeyEvent) {
    if state.loading {
        return;
    }
    if key.code == KeyCode::Esc {
        app.view = AppView::TemplatesPanel;
        return;
    }

    if key.code == KeyCode::Tab {
        state.active_field = (state.active_field + 1) % 2;
        update_field_styles(state);
        return;
    }

    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        state.loading = true;
        submit_ticket(app, state);
        return;
    }

    match state.active_field {
        0 => { state.summary_input.input(key); }
        1 => { state.description_input.input(key); }
        _ => {}
    }
}

fn update_field_styles(state: &mut CreateState) {
    let active = Style::default().fg(Color::Yellow);
    let inactive = Style::default().fg(Color::DarkGray);

    state.summary_input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Summary ")
            .border_style(if state.active_field == 0 { active } else { inactive }),
    );
    state.description_input.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Description ")
            .border_style(if state.active_field == 1 { active } else { inactive }),
    );
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

    frame.render_widget(&state.summary_input, chunks[2]);
    frame.render_widget(&state.description_input, chunks[3]);

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
