use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEventKind},
    execute,
    style::{Attribute, ResetColor, SetAttribute},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc;

mod app;
mod config;
pub mod data;
pub mod domain;
mod input;
mod ui;

use app::{App, AppCommand, FocusPane};
use config::WORLD_CUP_2026;
use data::{
    repository::Repository,
    sqlite::SqliteRepository,
    sync::{RefreshCoordinator, RefreshEvent, RefreshReason, RefreshRequest, ResourceKey},
};

type Tui = Terminal<CrosstermBackend<Stdout>>;
const BACKGROUND_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let database_info = data::sqlite::initialize()?;
    let db_path = database_info.path.clone();
    let (refresh_event_sender, refresh_event_receiver) = mpsc::unbounded_channel();
    let refresh_coordinator =
        RefreshCoordinator::start(db_path, WORLD_CUP_2026, refresh_event_sender);
    let mut terminal = init_terminal()?;
    let result = run(
        &mut terminal,
        database_info,
        refresh_coordinator,
        refresh_event_receiver,
    );
    restore_terminal(&mut terminal)?;
    result
}

fn run(
    terminal: &mut Tui,
    database_info: data::sqlite::DatabaseInfo,
    refresh_coordinator: RefreshCoordinator,
    mut refresh_event_receiver: mpsc::UnboundedReceiver<data::sync::RefreshEvent>,
) -> Result<()> {
    let mut app = App::new(WORLD_CUP_2026);
    app.set_database_info(database_info);
    let db_path = app
        .database_info()
        .map(|info| info.path.clone())
        .expect("database info is set before snapshot load");
    load_snapshot(&mut app, &db_path);
    request_refresh(
        &refresh_coordinator,
        app.startup_refresh_resources(),
        RefreshReason::Startup,
    );
    let mut next_background_refresh = Instant::now() + BACKGROUND_REFRESH_INTERVAL;
    let mut needs_draw = true;
    let mut panel_metrics = ui::PanelMetrics::default();

    while app.is_running() {
        let db_path = app
            .database_info()
            .map(|info| info.path.clone())
            .expect("database info is set before run loop");
        if drain_refresh_events(&mut app, &mut refresh_event_receiver, &db_path) {
            needs_draw = true;
        }

        if Instant::now() >= next_background_refresh {
            request_refresh(
                &refresh_coordinator,
                app.background_refresh_resources(),
                RefreshReason::Policy,
            );
            next_background_refresh = Instant::now() + BACKGROUND_REFRESH_INTERVAL;
        }

        if needs_draw {
            execute!(
                terminal.backend_mut(),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Hide
            )?;
            terminal.draw(|frame| {
                panel_metrics = ui::draw(frame, &app);
            })?;
            execute!(
                terminal.backend_mut(),
                SetAttribute(Attribute::Reset),
                ResetColor,
                Hide
            )?;
            needs_draw = false;
        }

        if event::poll(Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(command) = input::command_for_key(
                        key,
                        app.input_mode(),
                        app.key_scope(),
                        app.screen(),
                        app.focus_pane(),
                    ) {
                        let command = apply_scroll_max(command, app.focus_pane(), panel_metrics);
                        handle_command(command, &mut app, &refresh_coordinator);
                        needs_draw = true;
                    }
                }
                Event::Resize(_, _) => {
                    terminal.clear()?;
                    needs_draw = true;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn drain_refresh_events(
    app: &mut App,
    refresh_event_receiver: &mut mpsc::UnboundedReceiver<data::sync::RefreshEvent>,
    db_path: &std::path::Path,
) -> bool {
    let mut handled_event = false;
    while let Ok(event) = refresh_event_receiver.try_recv() {
        handled_event = true;
        let should_reload = matches!(
            &event,
            RefreshEvent::Succeeded {
                resource: ResourceKey::Teams
                    | ResourceKey::Matches
                    | ResourceKey::StandingsGroup(_)
                    | ResourceKey::Timeline(_),
                ..
            }
        );
        app.handle_refresh_event(event);
        if should_reload {
            load_snapshot(app, db_path);
        }
    }
    handled_event
}

fn load_snapshot(app: &mut App, db_path: &std::path::Path) {
    let repository = SqliteRepository::new(db_path);
    match repository.load_snapshot() {
        Ok(snapshot) => {
            app.set_snapshot(snapshot);
            match data::sqlite::latest_data_updated_at(db_path) {
                Ok(last_sync) => app.set_last_sync(last_sync),
                Err(error) => app.set_message(format!("Loading sync metadata failed: {error}")),
            }
        }
        Err(error) => app.set_snapshot_error(error.to_string()),
    }
}

fn handle_command(command: AppCommand, app: &mut App, refresh_coordinator: &RefreshCoordinator) {
    if matches!(command, AppCommand::Refresh) {
        let resources = app.refresh_resources();
        app.handle_command(command);
        if !resources.is_empty() {
            request_refresh(refresh_coordinator, resources, RefreshReason::Manual);
        }
        return;
    }

    if matches!(command, AppCommand::ToggleSelectedCountryFavorite) {
        let Some((team_id, team_name, was_favorite)) = app.selected_country_action() else {
            app.handle_command(command);
            return;
        };
        let Some(db_path) = app.database_info().map(|info| info.path.clone()) else {
            app.handle_command(command);
            return;
        };

        let repository = SqliteRepository::new(&db_path);
        match repository.toggle_favorite_team(team_id) {
            Ok(()) => {
                load_snapshot(app, &db_path);
                let state = if was_favorite { "Removed" } else { "Added" };
                app.set_message(format!("{state} favorite: {team_name}."));
            }
            Err(error) => {
                app.set_message(format!("Favorite toggle failed: {error}"));
            }
        }
        return;
    }

    if matches!(command, AppCommand::OpenDetails) {
        let timeline_resource = if app.screen() == app::Screen::Matches {
            (!app.selected_match_is_future())
                .then(|| app.selected_match_id().map(ResourceKey::Timeline))
                .flatten()
        } else {
            None
        };

        app.handle_command(command);
        if let Some(resource) = timeline_resource {
            request_refresh(
                refresh_coordinator,
                vec![resource],
                RefreshReason::ScreenEnter,
            );
        }
        return;
    }

    app.handle_command(command);
}

fn request_refresh(
    refresh_coordinator: &RefreshCoordinator,
    resources: Vec<ResourceKey>,
    reason: RefreshReason,
) {
    if resources.is_empty() {
        return;
    }

    let _ = refresh_coordinator.request(RefreshRequest {
        resources,
        reason,
        force: reason == RefreshReason::Manual,
    });
}

fn apply_scroll_max(
    command: AppCommand,
    focus: FocusPane,
    panel_metrics: ui::PanelMetrics,
) -> AppCommand {
    let max = match focus {
        FocusPane::Content => panel_metrics.content_max_scroll,
        FocusPane::Detail => panel_metrics.detail_max_scroll,
        FocusPane::None => 0,
    };
    let visible_lines = match focus {
        FocusPane::Content => panel_metrics.content_visible_lines,
        FocusPane::Detail => panel_metrics.detail_visible_lines,
        FocusPane::None => 0,
    };
    let max_horizontal = match focus {
        FocusPane::Content => panel_metrics.content_max_horizontal_scroll,
        FocusPane::Detail => panel_metrics.detail_max_horizontal_scroll,
        FocusPane::None => 0,
    };

    match command {
        AppCommand::ScrollUp { .. } => AppCommand::ScrollUp { max, visible_lines },
        AppCommand::ScrollDown { .. } => AppCommand::ScrollDown { max, visible_lines },
        AppCommand::ScrollLeft { .. } => AppCommand::ScrollLeft {
            max: max_horizontal,
        },
        AppCommand::ScrollRight { .. } => AppCommand::ScrollRight {
            max: max_horizontal,
        },
        command => command,
    }
}

fn init_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        SetAttribute(Attribute::Reset),
        ResetColor,
        Hide
    )?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Tui) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        SetAttribute(Attribute::Reset),
        ResetColor,
        Show,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;
    Ok(())
}
