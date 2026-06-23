use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
};

use crate::app::{App, Screen};

mod components;
mod countries;
mod facts;
mod home;
mod knockouts;
mod matches;
mod standings;
mod stats;

pub use components::PanelMetrics;

pub fn draw(frame: &mut Frame<'_>, app: &App) -> PanelMetrics {
    let size = frame.area();
    let shell = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(10)])
        .split(size);

    render_header(frame, shell[0], app);
    let metrics = render_body(frame, shell[1], app);

    if app.help_open() {
        render_help_overlay(frame, centered_rect(72, 72, size));
    }

    if app.quit_confirm_open() {
        render_quit_confirm_overlay(frame, centered_rect(46, 26, size));
    }

    metrics
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let config = app.config();
    let title = Line::from(vec![
        Span::styled(
            "footui",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("/"),
        Span::styled(config.name, Style::default().fg(Color::White)),
    ]);

    let author = Line::from(vec![
        Span::raw("author: "),
        Span::styled(
            "Ale G <alejoegarcia.dev>",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let block = Block::default().borders(Borders::BOTTOM);
    let inner = block.inner(area);

    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(title).alignment(Alignment::Left), inner);
    frame.render_widget(Paragraph::new(author).alignment(Alignment::Right), inner);
}
fn render_body(frame: &mut Frame<'_>, area: Rect, app: &App) -> PanelMetrics {
    if area.width >= 100 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(24), Constraint::Min(40)])
            .split(area);

        render_nav_rail(frame, columns[0], app);
        render_screen(frame, columns[1], app)
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(1),
            ])
            .split(area);

        render_top_tabs(frame, rows[0], app);
        let metrics = render_screen(frame, rows[1], app);
        components::render_compact_status_bar(frame, rows[2], app);
        metrics
    }
}

fn render_nav_rail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(8)])
        .split(area);

    let lines: Vec<Line<'_>> = Screen::ALL
        .iter()
        .map(|screen| {
            let selected = *screen == app.screen();
            let style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };

            if selected {
                Line::from(vec![
                    Span::styled(format!(">{}", screen.key_hint()), style),
                    Span::raw(" "),
                    Span::styled(screen.title(), style),
                ])
            } else {
                Line::from(vec![
                    Span::styled(screen.key_hint(), Style::default().fg(Color::DarkGray)),
                    Span::raw("  "),
                    Span::styled(screen.title(), style),
                ])
            }
        })
        .collect();

    let nav = Paragraph::new(lines)
        .block(Block::default().title("Screens").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(nav, chunks[0]);

    components::render_state_panel(frame, chunks[1], app);
}

fn render_top_tabs(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let titles: Vec<Line<'_>> = Screen::ALL
        .iter()
        .map(|screen| Line::from(format!("{} {}", screen.key_hint(), screen.title())))
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.screen().index())
        .block(Block::default().borders(Borders::BOTTOM))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn render_screen(frame: &mut Frame<'_>, area: Rect, app: &App) -> PanelMetrics {
    let has_room_for_detail = area.width >= 100 && area.height >= 24;
    let chunks = screen_chunks(area, app, has_room_for_detail);

    match app.screen() {
        Screen::Home => home::render(frame, area, app),
        Screen::Countries => countries::render(frame, chunks[0], app).merge(
            countries::render_detail(frame, chunks[1], app, components::detail_focused(app)),
        ),
        Screen::Matches => matches::render(frame, chunks[0], app).merge(matches::render_detail(
            frame,
            chunks[1],
            app,
            components::detail_focused(app),
        )),
        Screen::Standings => standings::render(frame, area, app),
        Screen::Knockouts => knockouts::render(frame, area, app),
        Screen::Stats => {
            stats::render(frame, chunks[0], app).merge(components::render_detail_panel(
                frame,
                chunks[1],
                app,
                "Stat details",
                components::detail_focused(app),
            ))
        }
        Screen::Facts => {
            facts::render(frame, chunks[0], app).merge(components::render_detail_panel(
                frame,
                chunks[1],
                app,
                "Fact details",
                components::detail_focused(app),
            ))
        }
    }
}

fn screen_chunks(area: Rect, app: &App, has_room_for_detail: bool) -> std::rc::Rc<[Rect]> {
    if has_room_for_detail {
        let constraints = if app.screen() == Screen::Matches && app.detail_open() {
            [Constraint::Percentage(34), Constraint::Percentage(66)]
        } else {
            [Constraint::Percentage(64), Constraint::Percentage(36)]
        };

        return Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);
    }

    let constraints = if app.screen() == Screen::Matches && app.detail_open() {
        [Constraint::Length(8), Constraint::Min(10)]
    } else {
        [Constraint::Min(8), Constraint::Length(7)]
    };

    Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area)
}

fn render_help_overlay(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(Span::styled(
            "Global keys",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("- 0 Home | 1 Countries | 2 Matches | 3 Standings | 4 Knock-outs"),
        Line::from("- 5 Stats | 6 Fun Facts (TBD)"),
        Line::from("- Space toggles global/screen key scope"),
        Line::from("- s/r sync | f favorites filter | t time mode"),
        Line::from("- q quit | h/? help | Esc close overlay/detail/search"),
        Line::from("- Tab / Shift+Tab focus panels"),
        Line::from("- Focused panels: Up/Down scroll, Left/Right pan where available"),
        Line::from("- s/q starts search where supported"),
        Line::from("- Enter opens details"),
    ];

    let overlay = Paragraph::new(lines)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(overlay, area);
}

fn render_quit_confirm_overlay(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(vec![Span::styled(
            "Quit footui?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from("Press q, y, or Enter to quit."),
        Line::from("Press n or Esc to keep working."),
    ];

    let overlay = Paragraph::new(lines)
        .block(Block::default().title("Confirm").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    frame.render_widget(overlay, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn screen_title_with_count(name: &'static str, shown: usize, total: usize) -> String {
    return format!("{name} ({shown}/{total})");
}
