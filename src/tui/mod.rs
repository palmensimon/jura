pub mod app;
pub mod views;

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{
            self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste,
            EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind,
        },
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};
use std::io;
use tokio::sync::mpsc;

use app::{App, AppEvent, AppView, Tab};
use views::{
    create_ticket::{self, CreateState},
    filter_panel::{self, FilterPanelResult, FilterPanelState},
    help,
    settings::{self, SettingsState},
    templates_panel::{self, TemplatesPanelResult, TemplatesPanelState},
    ticket_detail::{self, BranchPickState, DetailState},
    ticket_list,
    ticket_search::{self, TicketSearchState},
    transition_picker::{self, TransitionState},
};

use crate::{
    config::{Config, DefaultFilter, Templates, config_dir, save_settings},
    git::{branch_name, find_branches_for_ticket, new_pr_url, open_url},
    jira::JiraClient,
};

pub async fn run_tui(config: Config, templates: Templates, client: JiraClient) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (event_tx, mut event_rx) = mpsc::channel(64);
    let mut app = App::new(config, templates.templates, client, event_tx);
    let mut create_state = CreateState::new();
    let mut settings_state = SettingsState::new(&app.config);
    let mut filter_panel_state = FilterPanelState::new(&app.filter);
    let mut templates_panel_state = TemplatesPanelState::new();
    let mut transition_state = TransitionState::new();
    let mut detail_state = DetailState::new();
    let mut ticket_search_state = TicketSearchState::new(
        app.config.project.as_deref().or(app.filter.project.as_deref()),
        AppView::TicketList,
    );

    app.trigger_load();

    // Load current user for assignment toggle
    {
        let client = app.client.clone();
        let tx = app.event_tx.clone();
        tokio::spawn(async move {
            if let Ok(user) = client.get_myself().await {
                if let Some(name) = user.name {
                    let _ = tx.send(AppEvent::UserLoaded(name)).await;
                }
            }
        });
    }

    loop {
        terminal.draw(|frame| {
            let full_area = frame.area();
            let (content_area, bar_area) = help::split_with_bar(full_area);

            // Draw the active view in the content area
            match &app.view {
                AppView::TicketList => {
                    ticket_list::draw(&mut app, frame, content_area);
                    match &detail_state.branch_pick {
                        BranchPickState::Editing { .. } => ticket_detail::draw_branch_editor(&mut detail_state, frame, content_area),
                        BranchPickState::Picking { .. } => ticket_detail::draw_branch_picker(&detail_state, frame, content_area),
                        BranchPickState::Idle => {}
                    }
                }
                AppView::TicketDetail { .. } => ticket_detail::draw(&app, &mut detail_state, frame, content_area),
                AppView::CreateTicket => {
                    create_ticket::draw(&app, &mut create_state, frame, content_area)
                }
                AppView::Settings => settings::draw(&app, &mut settings_state, frame, content_area),
                AppView::FilterPanel => {
                    // Draw ticket list as background, then overlay the popup
                    ticket_list::draw(&mut app, frame, content_area);
                    filter_panel::draw(&app, &mut filter_panel_state, frame, content_area);
                }
                AppView::TemplatesPanel => {
                    ticket_list::draw(&mut app, frame, content_area);
                    templates_panel::draw(&app, &templates_panel_state, frame, content_area);
                }
                AppView::TransitionPicker { .. } => {
                    let from_list = transition_state.return_to_list;
                    if from_list {
                        ticket_list::draw(&mut app, frame, content_area);
                    } else {
                        ticket_detail::draw(&app, &mut detail_state, frame, content_area);
                    }
                    transition_picker::draw(&app, &mut transition_state, frame, content_area);
                }
                AppView::TicketSearch => {
                    match &ticket_search_state.prev_view {
                        AppView::TicketDetail { .. } => ticket_detail::draw(&app, &mut detail_state, frame, content_area),
                        _ => ticket_list::draw(&mut app, frame, content_area),
                    }
                    ticket_search::draw(&ticket_search_state, frame, content_area);
                }
            }

            // Global bottom status bar
            if matches!(app.view, AppView::TicketList) && detail_state.is_picking() {
                ticket_detail::draw_bar(&app, &detail_state, frame, bar_area);
            } else if matches!(app.view, AppView::TicketList) {
                ticket_list::draw_bar(&app, frame, bar_area);
            } else if matches!(app.view, AppView::TicketDetail { .. }) {
                ticket_detail::draw_bar(&app, &detail_state, frame, bar_area);
            } else {
                help::draw_status_bar(frame, bar_area, help::status_bar_hints(&app.view), app.all.loading || app.mine.loading, app.status_msg.as_deref());
            }

            // Help popup (drawn last so it's on top)
            if app.show_help {
                help::draw(frame, full_area, app.help_scroll);
            }
        })?;

        tokio::select! {
            Some(app_event) = event_rx.recv() => {
                let was_picking = matches!(app.view, AppView::TransitionPicker { .. });
                let is_applied = matches!(app_event, AppEvent::TransitionApplied(_));
                app.handle_event(app_event);
                // Refresh settings state when config changes
                if matches!(app.view, AppView::Settings) {
                    settings_state = SettingsState::new(&app.config);
                }
                if was_picking {
                    if is_applied {
                        app.view = if transition_state.return_to_list {
                            AppView::TicketList
                        } else if let AppView::TransitionPicker { issue } = &app.view {
                            AppView::TicketDetail { issue: issue.clone() }
                        } else {
                            AppView::TicketList
                        };
                        transition_state = TransitionState::new();
                    } else if app.error.is_some() {
                        transition_state.loading = false;
                    }
                }
                if app.error.is_some() {
                    create_state.loading = false;
                    ticket_search_state.loading = false;
                }
            }
            poll_result = tokio::task::spawn_blocking(|| event::poll(std::time::Duration::from_millis(50))) => {
                if !matches!(poll_result, Ok(Ok(true))) {
                    continue; // timeout or error — redraw and wait again without blocking
                }
                match event::read() {
                    Ok(Event::Paste(ref text)) => {
                        if matches!(app.view, AppView::CreateTicket) {
                            create_ticket::handle_paste(&mut create_state, text);
                        }
                        continue;
                    }
                    Ok(Event::Mouse(mouse)) => {
                        let scroll_lines = 3usize;
                        match mouse.kind {
                            MouseEventKind::ScrollUp => {
                                if matches!(app.view, AppView::TicketList) {
                                    for _ in 0..scroll_lines { app.move_selection_up(); }
                                }
                            }
                            MouseEventKind::ScrollDown => {
                                if matches!(app.view, AppView::TicketList) {
                                    for _ in 0..scroll_lines { app.move_selection_down(); }
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    // Global: ? toggles help (suppress in text-input views)
                    let in_text_input = matches!(app.view, AppView::CreateTicket)
                        || (matches!(app.view, AppView::Settings) && settings_state.is_editing());
                    if key.code == KeyCode::Char('?') && !in_text_input {
                        app.show_help = !app.show_help;
                        if app.show_help { app.help_scroll = 0; }
                        continue;
                    }
                    if app.show_help {
                        match key.code {
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.help_scroll = app.help_scroll.saturating_sub(1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.help_scroll = app.help_scroll.saturating_add(1);
                            }
                            KeyCode::PageUp => {
                                app.help_scroll = app.help_scroll.saturating_sub(10);
                            }
                            KeyCode::PageDown => {
                                app.help_scroll = app.help_scroll.saturating_add(10);
                            }
                            _ => { app.show_help = false; }
                        }
                        continue;
                    }

                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        break;
                    }

                    if key.code == KeyCode::Char('k') && key.modifiers.contains(KeyModifiers::CONTROL)
                        && !matches!(app.view, AppView::TicketSearch)
                    {
                        let prev = app.view.clone();
                        let project = app.config.project.as_deref().or(app.filter.project.as_deref());
                        ticket_search_state = TicketSearchState::new(project, prev);
                        app.view = AppView::TicketSearch;
                        continue;
                    }

                    if key.code == KeyCode::Char('q') {
                        if matches!(app.view, AppView::TicketList) && !app.active_tab().local_search_active {
                            break;
                        }
                    }

                    app.status_msg = None;
                    if key.code != KeyCode::Char('q') {
                        app.error = None;
                    }

                    let open_in_nvim = |path: std::path::PathBuf| {
                        disable_raw_mode().ok();
                        execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture).ok();
                        let _ = std::process::Command::new("nvim").arg(&path).status();
                        enable_raw_mode().ok();
                        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture).ok();
                    };

                    match app.view.clone() {
                        AppView::TicketList => {
                            if matches!(detail_state.branch_pick, BranchPickState::Editing { .. }) {
                                ticket_detail::handle_branch_editor_key(&mut app, &mut detail_state, key);
                            } else if matches!(detail_state.branch_pick, BranchPickState::Picking { .. }) {
                                ticket_detail::handle_branch_picker_key(&mut app, &mut detail_state, key);
                            } else if key.code == KeyCode::Char('s') && !app.active_tab().local_search_active {
                                settings_state = SettingsState::new(&app.config);
                                app.view = AppView::Settings;
                            } else if key.code == KeyCode::Char('t') && !app.active_tab().local_search_active {
                                if let Some(issue) = app.selected_issue().cloned() {
                                    let key_str = issue.key.clone();
                                    if let Some(cached) = crate::cache::storage::load_transition_cache(&key_str) {
                                        app.available_transitions = cached;
                                    } else {
                                        app.available_transitions.clear();
                                    }
                                    transition_state = TransitionState::new();
                                    transition_state.return_to_list = true;
                                    app.view = AppView::TransitionPicker { issue: Box::new(issue) };
                                    let client = app.client.clone();
                                    let tx = app.event_tx.clone();
                                    tokio::spawn(async move {
                                        match client.get_transitions(&key_str).await {
                                            Ok(t) => { let _ = tx.send(AppEvent::TransitionsLoaded(t, key_str)).await; }
                                            Err(e) => { let _ = tx.send(AppEvent::Error(format!("{e:#}"))).await; }
                                        }
                                    });
                                }
                            } else if key.code == KeyCode::Char('C') && !app.active_tab().local_search_active {
                                if let Some(issue) = app.selected_issue().cloned() {
                                    let branches = find_branches_for_ticket(&issue.key);
                                    if branches.is_empty() {
                                        let suggested = branch_name(&issue.key, issue.summary());
                                        let mut ta = tui_textarea::TextArea::from([suggested.as_str()]);
                                        ta.move_cursor(tui_textarea::CursorMove::End);
                                        detail_state.branch_pick = BranchPickState::Editing { input: ta, issue };
                                    } else {
                                        detail_state.branch_pick = BranchPickState::Picking { branches, selected: 0, issue };
                                    }
                                }
                            } else if key.code == KeyCode::Char('c') && !app.active_tab().local_search_active {
                                if let Some(issue) = app.selected_issue().cloned() {
                                    let branches = find_branches_for_ticket(&issue.key);
                                    match branches.len() {
                                        0 => {
                                            let suggested = branch_name(&issue.key, issue.summary());
                                            let mut ta = tui_textarea::TextArea::from([suggested.as_str()]);
                                            ta.move_cursor(tui_textarea::CursorMove::End);
                                            detail_state.branch_pick = BranchPickState::Editing { input: ta, issue };
                                        }
                                        1 => app.spawn_checkout(branches.into_iter().next().unwrap(), &issue),
                                        _ => detail_state.branch_pick = BranchPickState::Picking { branches, selected: 0, issue },
                                    }
                                }
                            } else if key.code == KeyCode::Char('a') && !app.active_tab().local_search_active {
                                if let Some(issue) = app.selected_issue().cloned() {
                                    app.toggle_assignment(&issue);
                                }
                            } else if key.code == KeyCode::Char('b') && !app.active_tab().local_search_active {
                                if let Some(issue) = app.selected_issue() {
                                    let url = format!("{}/browse/{}", app.config.jira.base_url, issue.key);
                                    let _ = open_url(&url, app.config.defaults.browser.as_deref());
                                }
                            } else if key.code == KeyCode::Char('o') && !app.active_tab().local_search_active {
                                if let Some(issue) = app.selected_issue() {
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
                            } else if key.code == KeyCode::Char('n') && !app.active_tab().local_search_active {
                                templates_panel_state = TemplatesPanelState::new();
                                app.view = AppView::TemplatesPanel;
                            } else if key.code == KeyCode::Char('f') && !app.active_tab().local_search_active {
                                filter_panel_state = FilterPanelState::new(&app.filter);
                                app.view = AppView::FilterPanel;
                                // Fetch statuses + components in background
                                let client = app.client.clone();
                                let project = app
                                    .filter
                                    .project
                                    .clone()
                                    .or_else(|| app.config.project.clone());
                                let tx = app.event_tx.clone();
                                tokio::spawn(async move {
                                    let statuses =
                                        client.get_statuses().await.unwrap_or_default();
                                    let components = if let Some(p) = project {
                                        client
                                            .get_project_components(&p)
                                            .await
                                            .map(|cs| cs.into_iter().map(|c| c.name).collect())
                                            .unwrap_or_default()
                                    } else {
                                        vec![]
                                    };
                                    let _ = tx
                                        .send(AppEvent::FilterOptions { statuses, components })
                                        .await;
                                });
                            } else {
                                ticket_list::handle_key(&mut app, key);
                            }
                        }
                        AppView::TicketDetail { .. } => {
                            if key.code == KeyCode::Char('t') && matches!(detail_state.branch_pick, BranchPickState::Idle) {
                                if let AppView::TicketDetail { issue } = &app.view {
                                    let issue = issue.clone();
                                    let key_str = issue.key.clone();
                                    // Pre-populate from cache so the list is instant
                                    if let Some(cached) = crate::cache::storage::load_transition_cache(&key_str) {
                                        app.available_transitions = cached;
                                    } else {
                                        app.available_transitions.clear();
                                    }
                                    transition_state = TransitionState::new();
                                    app.view = AppView::TransitionPicker { issue };
                                    let client = app.client.clone();
                                    let tx = app.event_tx.clone();
                                    tokio::spawn(async move {
                                        match client.get_transitions(&key_str).await {
                                            Ok(t) => {
                                                let _ = tx
                                                    .send(AppEvent::TransitionsLoaded(t, key_str))
                                                    .await;
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(AppEvent::Error(format!("{e:#}")))
                                                    .await;
                                            }
                                        }
                                    });
                                }
                            } else {
                                ticket_detail::handle_key(&mut app, &mut detail_state, key);
                            }
                        }
                        AppView::TransitionPicker { .. } => {
                            transition_picker::handle_key(&mut app, &mut transition_state, key);
                        }
                        AppView::CreateTicket => {
                            if key.code == KeyCode::Char('e') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                let current_content = match create_state.active_field {
                                    0 => create_state.summary_input.lines().join("\n"),
                                    1 => create_state.description_input.lines().join("\n"),
                                    _ => String::new(),
                                };
                                let tmp_path = std::env::temp_dir().join("jura_edit.tmp");
                                let _ = std::fs::write(&tmp_path, &current_content);
                                disable_raw_mode().ok();
                                execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture).ok();
                                let editor = std::env::var("VISUAL")
                                    .or_else(|_| std::env::var("EDITOR"))
                                    .unwrap_or_else(|_| "vi".to_string());
                                let _ = std::process::Command::new(&editor).arg(&tmp_path).status();
                                enable_raw_mode().ok();
                                execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture).ok();
                                terminal.clear().ok();
                                if let Ok(new_content) = std::fs::read_to_string(&tmp_path) {
                                    let lines: Vec<String> = new_content.lines().map(String::from).collect();
                                    match create_state.active_field {
                                        0 => {
                                            create_state.summary_input = tui_textarea::TextArea::from(lines);
                                        }
                                        1 => {
                                            create_state.description_input = tui_textarea::TextArea::from(lines);
                                        }
                                        _ => {}
                                    }
                                    create_ticket::update_field_styles(&mut create_state);
                                }
                            } else {
                                create_ticket::handle_key(&mut app, &mut create_state, key)
                            }
                        }
                        AppView::Settings => {
                            if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                open_in_nvim(config_dir().join("user_settings.yaml"));
                                terminal.clear().ok();
                            } else if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                open_in_nvim(config_dir().join("templates.yaml"));
                                terminal.clear().ok();
                            } else if key.code == KeyCode::Char('r') && !settings_state.is_editing() {
                                let mut reload_ok = true;
                                match crate::config::load_config() {
                                    Ok(new_cfg) => {
                                        app.filter = app::FilterState::from_config(&new_cfg);
                                        app.config = std::sync::Arc::new(new_cfg);
                                        settings_state = SettingsState::new(&app.config);
                                    }
                                    Err(e) => {
                                        app.error = Some(format!("Reload failed: {e:#}"));
                                        reload_ok = false;
                                    }
                                }
                                match crate::config::load_templates() {
                                    Ok(t) => { app.templates = t.templates; }
                                    Err(e) => {
                                        app.error = Some(format!("Template reload failed: {e:#}"));
                                        reload_ok = false;
                                    }
                                }
                                if reload_ok {
                                    app.status_msg = Some("Configuration reloaded".to_string());
                                }
                            } else {
                                settings::handle_key(&mut app, &mut settings_state, key);
                            }
                        }
                        AppView::TemplatesPanel => {
                            if key.code == KeyCode::Char('r') {
                                match crate::config::load_templates() {
                                    Ok(t) => {
                                        app.templates = t.templates;
                                        templates_panel_state.selected_idx = 0;
                                        app.status_msg = Some("Templates reloaded".to_string());
                                    }
                                    Err(e) => app.error = Some(format!("Template reload failed: {e:#}")),
                                }
                            } else if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
                                open_in_nvim(config_dir().join("templates.yaml"));
                                terminal.clear().ok();
                            } else {
                                let len = app.templates.len();
                                match templates_panel::handle_key(&mut templates_panel_state, len, key) {
                                    Some(TemplatesPanelResult::Selected(idx)) => {
                                        create_state = CreateState::new();
                                        create_state.template_idx = idx;
                                        app.view = AppView::CreateTicket;
                                    }
                                    Some(TemplatesPanelResult::Cancel) => {
                                        app.view = AppView::TicketList;
                                    }
                                    None => {}
                                }
                            }
                        }
                        AppView::TicketSearch => {
                            ticket_search::handle_key(&mut app, &mut ticket_search_state, key);
                        }
                        AppView::FilterPanel => {
                            match filter_panel::handle_key(&mut app, &mut filter_panel_state, key) {
                                Some(FilterPanelResult::Apply(filter)) => {
                                    app.filter = filter;
                                    app.view = AppView::TicketList;
                                    app.trigger_load_tab(Tab::All);
                                    app.trigger_load_tab(Tab::Mine);
                                }
                                Some(FilterPanelResult::Save(filter)) => {
                                    app.filter = filter.clone();
                                    app.view = AppView::TicketList;

                                    let mut new_cfg = (*app.config).clone();
                                    new_cfg.defaults.default_filter = DefaultFilter {
                                        statuses: filter.selected_statuses.clone(),
                                        hidden_statuses: filter.hidden_statuses.clone(),
                                        component: filter.component.clone(),
                                        labels: filter.labels.clone(),
                                        team: filter.team.clone(),
                                        epic: filter.epic.clone(),
                                        sprint_active_only: filter.sprint_active_only,
                                        sort_by: filter.sort_by.as_str().to_string(),
                                        sort_dir: filter.sort_dir.as_str().to_string(),
                                    };
                                    let tx = app.event_tx.clone();
                                    tokio::spawn(async move {
                                        match save_settings(&new_cfg.defaults) {
                                            Ok(()) => {
                                                let _ =
                                                    tx.send(AppEvent::FilterSaved(new_cfg)).await;
                                            }
                                            Err(e) => {
                                                let _ = tx
                                                    .send(AppEvent::Error(format!("{e:#}")))
                                                    .await;
                                            }
                                        }
                                    });

                                    app.trigger_load_tab(Tab::All);
                                    app.trigger_load_tab(Tab::Mine);
                                }
                                Some(FilterPanelResult::Cancel) => {
                                    app.view = AppView::TicketList;
                                }
                                None => {}
                            }
                        }
                    }
                    } // end key event
                    _ => {}
                } // end match event::read()
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture, DisableBracketedPaste)?;
    Ok(())
}
