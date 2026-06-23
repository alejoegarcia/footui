use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    app::CountriesFilter,
    ui::{components, screen_title_with_count},
};

use crate::app::App;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let filter = app.country_filter_label();
    let teams = app.visible_teams();
    let title = screen_title_with_count("Standings", teams.len(), app.team_count());
    let header = vec![
        components::filter_line(filter.clone()),
        components::sort_line_for_scope(
            "by country name",
            app.countries_sort().label(),
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "Filters: ",
            CountriesFilter::ALL
                .iter()
                .map(|filter| filter.menu_label()),
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "Actions: ",
            ["[q] search", "[*] favorite selected"],
            components::screen_scope_active(app),
        ),
    ];

    let name_width = team_name_width(&teams);
    let selected_country_id = app.selected_country_id();
    let mut lines = Vec::new();

    if app.team_count() == 0 {
        lines.extend([
            Line::from("No countries stored yet."),
            Line::from("Press s in global scope from this screen to sync teams from FIFA."),
        ]);
    } else if teams.is_empty() {
        lines.push(Line::from("No countries match the current filters."));
    } else {
        lines.push(Line::from(vec![Span::styled(
            format!(
                "{:<name_width$}  {:<4}  {:<10}  {:^3}",
                "Country", "Abbr", "Assoc", "Fav"
            ),
            Style::default().fg(Color::DarkGray),
        )]));
        for team in teams {
            let selected = selected_country_id.as_ref() == Some(&team.id);
            let style = Style::default().fg(Color::Gray);
            let selected_style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                style
            };
            lines.push(Line::from(vec![
                Span::styled(
                    country_cell(&team.name, name_width, selected),
                    selected_style,
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:<5}", team.abbreviation),
                    if selected {
                        selected_style
                    } else {
                        Style::default().fg(Color::Cyan)
                    },
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:<10}", team.confederation.code()),
                    if selected {
                        selected_style
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:^3}", if team.favorite { "*" } else { "" }),
                    if team.favorite {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        style
                    },
                ),
            ]));
        }
    }

    components::render_screen_frame(
        frame,
        area,
        &title,
        header,
        lines,
        components::content_focused(app),
        app.content_scroll(),
    )
}

pub fn render_detail(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    focused: bool,
) -> components::PanelMetrics {
    let lines = match app.selected_country() {
        Some(team) => vec![
            Line::from(vec![Span::styled(
                format!(
                    "{} ({}) - {} - {}",
                    team.name,
                    team.abbreviation,
                    team.confederation.code(),
                    if team.favorite {
                        "favorite"
                    } else {
                        "not a favorite"
                    }
                ),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            components::shortcut_menu_line_for_scope(
                "Actions: ",
                ["[*] toggle favorite"],
                components::screen_scope_active(app),
            ),
        ],
        None => vec![
            Line::from("No country selected."),
            Line::from("Sync countries first, then Tab to focus the list."),
        ],
    };

    let max_scroll = components::max_scroll(lines.len(), area);
    let scroll = app.detail_scroll().min(max_scroll);
    let title = components::scroll_title("Country details", scroll, max_scroll);
    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(components::focus_style(focused)),
        )
        .scroll((scroll, 0))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, area);

    components::PanelMetrics {
        content_max_scroll: 0,
        detail_max_scroll: max_scroll,
        content_max_horizontal_scroll: 0,
        detail_max_horizontal_scroll: 0,
        content_visible_lines: 0,
        detail_visible_lines: area.height.saturating_sub(2),
    }
}

fn team_name_width(teams: &[&crate::domain::Team]) -> usize {
    let data_width = teams
        .iter()
        .map(|team| team.name.chars().count())
        .max()
        .unwrap_or("Team".len())
        .max("Team".len());

    data_width + 1
}

fn country_cell(name: &str, width: usize, selected: bool) -> String {
    let value = if selected {
        format!(">{}", truncate(name, width.saturating_sub(1)))
    } else {
        truncate(name, width)
    };

    format!("{value:<width$}")
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    truncated.push('~');
    truncated
}
