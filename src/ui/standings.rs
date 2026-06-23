use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    app::App,
    domain::QualificationState,
    ui::{components, screen_title_with_count},
};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let filter = app.standing_group_label();
    let standings = app.visible_standings();
    let title = screen_title_with_count("Standings", standings.len(), app.standing_count());

    let header = vec![
        components::filter_line(filter.clone()),
        components::sort_line_for_scope(
            "by group letter",
            app.standings_sort().label(),
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "Groups: ",
            ["all g[r]oups", "Group [A-L]"],
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "Actions: ",
            ["[m]atches for focused group"],
            components::screen_scope_active(app),
        ),
    ];

    let mut lines = Vec::new();

    if app.standing_count() == 0 {
        lines.extend([
            Line::from("No standings stored yet."),
            Line::from("Press s in global scope from this screen to sync FIFA group tables."),
        ]);
    } else if standings.is_empty() {
        lines.push(Line::from("No standings match the current filters."));
    } else {
        lines.extend(standing_lines(app, &standings));
    }

    components::render_screen_frame_unwrapped(
        frame,
        area,
        &title,
        header,
        lines,
        components::content_focused(app),
        app.content_scroll(),
    )
}

fn standing_lines(app: &App, standings: &[&crate::domain::StandingRow]) -> Vec<Line<'static>> {
    let country_width = country_width(standings);
    let selected_group_id = app.selected_standing_group_id();
    let mut lines = Vec::new();
    let mut current_group_id: Option<&crate::domain::GroupId> = None;

    for row in standings {
        if current_group_id != Some(&row.group_id) {
            if current_group_id.is_some() {
                lines.push(Line::from(""));
            }

            current_group_id = Some(&row.group_id);
            let selected = selected_group_id.as_ref() == Some(&row.group_id);
            let marker = if selected { ">" } else { " " };
            let style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            lines.push(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(row.group_name.clone(), style),
            ]));
            lines.push(table_header(country_width));
        }

        lines.push(standing_row_line(
            row,
            country_width,
            app.standing_team_is_favorite(&row.team_id),
            app.standing_row_is_advancing(row),
            app.standing_row_qualification_state(row),
        ));
    }

    lines
}

fn table_header(country_width: usize) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!(
            "{:<country_width$}  {:>1}  {:>1}  {:>1}  {:>1}  {:>2}  {:>2}  {:>3}  {:>3}  {:>3}  {:^3}",
            "Country", "P", "W", "D", "L", "GF", "GA", "GD", "TCS", "Pts", "Fav"
        ),
        Style::default().fg(Color::DarkGray),
    )])
}

fn standing_row_line(
    row: &crate::domain::StandingRow,
    country_width: usize,
    favorite: bool,
    advancing: bool,
    qualification_state: QualificationState,
) -> Line<'static> {
    let rank = format!("#{}", row.position);
    let rank_width = rank.chars().count();
    let country_width = country_width.saturating_sub(rank_width);

    Line::from(vec![
        Span::styled(rank, rank_style(advancing, qualification_state)),
        Span::styled(
            format!(
                "{:<country_width$}",
                truncate(&format!(" {}", row.team_name), country_width)
            ),
            team_name_style(qualification_state),
        ),
        Span::styled(
            format!(
                "  {:>1}  {:>1}  {:>1}  {:>1}  {:>2}  {:>2}  {:>3}  {:>3}  {:>3}",
                row.played,
                row.won,
                row.drawn,
                row.lost,
                row.goals_for,
                row.goals_against,
                signed_number(row.goal_difference),
                optional_number(row.fair_play),
                row.points
            ),
            Style::default().fg(Color::White),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{:^3}", if favorite { "*" } else { "" }),
            if favorite {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            },
        ),
    ])
}

fn rank_style(advancing: bool, qualification_state: QualificationState) -> Style {
    match qualification_state {
        QualificationState::Disqualified => {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        }
        QualificationState::Qualified if advancing => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        _ if advancing => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Gray),
    }
}

fn team_name_style(qualification_state: QualificationState) -> Style {
    match qualification_state {
        QualificationState::Disqualified => {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        }
        QualificationState::Qualified => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        QualificationState::Open => Style::default().fg(Color::Gray),
    }
}

fn country_width(standings: &[&crate::domain::StandingRow]) -> usize {
    standings
        .iter()
        .map(|row| {
            format!("#{} {}", row.position, row.team_name)
                .chars()
                .count()
        })
        .max()
        .unwrap_or("Country".len())
        .max("Country".len())
}

fn signed_number(value: i16) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

fn optional_number(value: Option<i16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disqualified_standings_rows_are_red() {
        assert_eq!(
            rank_style(false, QualificationState::Disqualified).fg,
            Some(Color::Red)
        );
        assert_eq!(
            team_name_style(QualificationState::Disqualified).fg,
            Some(Color::Red)
        );
    }
}
