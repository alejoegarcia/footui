use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{app::App, ui::screen_title_with_count};
use crate::{
    app::{MatchesFilter, TimelineFilter},
    ui::components,
};

const TIME_WIDTH: usize = 18;
const CONTEXT_WIDTH: usize = 18;
const RESULT_WIDTH: usize = 16;
const STATUS_WIDTH: usize = 10;
const TIMELINE_MINUTE_WIDTH: usize = 6;
const TIMELINE_MINUTE_BAND_WIDTH: usize = TIMELINE_MINUTE_WIDTH + 2;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let filter = format!(
        "date: {} | groups: {}",
        app.matches_filter().label(),
        app.match_group_label()
    );
    let matches = app.visible_matches();
    let selected_match_id = app.selected_match_id();
    let visible_rows = if app.detail_open() {
        matches
            .iter()
            .copied()
            .filter(|match_| selected_match_id.as_ref() == Some(&match_.id))
            .collect::<Vec<_>>()
    } else {
        matches.clone()
    };
    let title = screen_title_with_count("Matches", visible_rows.len(), matches.len());

    let header = vec![
        components::filter_line(filter),
        components::shortcut_menu_line_for_scope(
            "Dates: ",
            MatchesFilter::ALL.iter().map(|filter| filter.menu_label()),
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "Groups: ",
            ["all g[r]oups", "Group [A-L]"],
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "Actions: ",
            ["Enter details"],
            components::screen_scope_active(app),
        ),
    ];

    let mut lines = Vec::new();

    if app.match_count() == 0 {
        lines.extend([
            Line::from("No matches stored yet."),
            Line::from("Press s in global scope from this screen to sync fixtures from FIFA."),
        ]);
    } else if matches.is_empty() {
        lines.push(Line::from("No matches match the current filters."));
    } else {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:<TIME_WIDTH$} ", "Time"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<CONTEXT_WIDTH$}", "Stage/Group"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<RESULT_WIDTH$}", "Result"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:<STATUS_WIDTH$}", "Status"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{:^3}", "Fav"),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        let mut previous_day: Option<String> = None;
        let mut day_color_index = 0;
        for match_ in visible_rows {
            let day_key = app.match_date_key(match_);
            if previous_day
                .as_deref()
                .is_some_and(|previous| previous != day_key)
            {
                day_color_index = 1 - day_color_index;
            }
            previous_day = Some(day_key);

            let selected = selected_match_id.as_ref() == Some(&match_.id);
            let time_color = match_day_color(day_color_index);
            let status_style = match match_.status {
                crate::domain::MatchStatus::Live => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                crate::domain::MatchStatus::FullTime => Style::default().fg(Color::DarkGray),
                crate::domain::MatchStatus::Scheduled => Style::default().fg(Color::Gray),
                _ => Style::default().fg(Color::Yellow),
            };
            lines.push(Line::from(vec![
                Span::styled(
                    format!(
                        "{} ",
                        time_cell(&app.match_time_label(match_), TIME_WIDTH, selected)
                    ),
                    selected_style(Style::default().fg(time_color), selected),
                ),
                Span::styled(
                    format!(
                        "{:<CONTEXT_WIDTH$}",
                        truncate(&match_context(match_), CONTEXT_WIDTH)
                    ),
                    selected_style(Style::default().fg(Color::White), selected),
                ),
                match_home_span(app, match_, selected),
                Span::styled(
                    format!(" {:^5} ", score_label(match_)),
                    selected_style(Style::default().fg(Color::Gray), selected),
                ),
                match_away_span(app, match_, selected),
                Span::raw(" "),
                Span::styled(
                    format!(
                        "{:<STATUS_WIDTH$}",
                        truncate(app.match_status_label(match_.status), STATUS_WIDTH)
                    ),
                    selected_style(status_style, selected),
                ),
                Span::styled(
                    format!(
                        "{:^3}",
                        if app.match_includes_favorite_team(match_) {
                            "*"
                        } else {
                            ""
                        }
                    ),
                    if app.match_includes_favorite_team(match_) {
                        selected_style(Style::default().fg(Color::Yellow), true)
                    } else {
                        selected_style(Style::default().fg(Color::Gray), selected)
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
    let lines = if !app.detail_open() {
        vec![
            Line::from("Press Enter to open match details."),
            Line::from("Future matches stay closed until data exists."),
        ]
    } else if app.selected_match_is_future() {
        vec![Line::from("Future matches have no details yet.")]
    } else if !app.selected_match_has_timeline() {
        vec![
            Line::from("Timeline data has not been downloaded for this match yet."),
            Line::from("Press Enter to request match details from FIFA."),
        ]
    } else {
        match app.selected_match() {
            Some(match_) => {
                match_detail_lines(app, match_, usize::from(area.width.saturating_sub(2)))
            }
            None => vec![
                Line::from("No match selected."),
                Line::from("Sync matches first, then Tab to focus the list."),
            ],
        }
    };

    let max_scroll = components::max_scroll(lines.len(), area);
    let scroll = app.detail_scroll().min(max_scroll);
    let title = components::scroll_title("Match details", scroll, max_scroll);
    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(components::focus_style(focused)),
        )
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false });
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

fn match_detail_lines(
    app: &App,
    match_: &crate::domain::Match,
    content_width: usize,
) -> Vec<Line<'static>> {
    let all_timeline_events = app.selected_match_timeline_events();
    let mut lines = vec![
        match_detail_header_line(app, match_, content_width, &all_timeline_events),
        Line::from(""),
        components::shortcut_menu_line_for_scope(
            "Detail filters: ",
            TimelineFilter::SELECTABLE
                .iter()
                .map(|filter| filter.menu_label()),
            components::screen_scope_active(app),
        ),
        Line::from(format!("Active filters: {}", app.timeline_filter_label())),
        Line::from("Timeline (most recent first):"),
    ];

    let timeline_events = all_timeline_events
        .into_iter()
        .filter(|event| timeline_event_matches_filters(event, app.timeline_filters()))
        .take(32)
        .collect::<Vec<_>>();

    if timeline_events.is_empty() {
        lines.push(Line::from(
            "No timeline events stored yet. Press Enter to fetch/open details.",
        ));
        return lines;
    }

    for (index, event) in timeline_events.iter().enumerate() {
        lines.extend(timeline_event_lines(match_, event, content_width));
        if index + 1 < timeline_events.len() {
            lines.push(timeline_spine_line(content_width));
        }
    }

    lines
}

fn match_detail_header_line(
    app: &App,
    match_: &crate::domain::Match,
    content_width: usize,
    events: &[&crate::domain::TimelineEvent],
) -> Line<'static> {
    let home_code = app.match_team_code(match_.home_team_id.as_ref(), &match_.home_team_name);
    let away_code = app.match_team_code(match_.away_team_id.as_ref(), &match_.away_team_name);
    let score = score_label(match_);
    let home_cards = card_totals_for_team(events, match_.home_team_id.as_ref());
    let away_cards = card_totals_for_team(events, match_.away_team_id.as_ref());
    let left = format!(
        "{home_code} {score} {away_code} | ({}-{}) / ({}-{})",
        home_cards.red, home_cards.yellow, away_cards.red, away_cards.yellow
    );
    let context = match_context(match_);
    let time_label = app.match_time_label(match_);
    let status_label = app.match_status_label(match_.status);
    let right = format!("{context} | {time_label} | {status_label}");
    let spacer_width = content_width
        .saturating_sub(left.chars().count() + right.chars().count())
        .max(1);

    Line::from(vec![
        Span::styled(
            home_code.clone(),
            team_style(winning_side(match_) == Some(MatchSide::Home)),
        ),
        Span::styled(format!(" {score} "), Style::default().fg(Color::Gray)),
        Span::styled(
            away_code.clone(),
            team_style(winning_side(match_) == Some(MatchSide::Away)),
        ),
        Span::styled(" | (".to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(
            home_cards.red.to_string(),
            TimelineEventKind::RedCard.style(),
        ),
        Span::raw("-"),
        Span::styled(
            home_cards.yellow.to_string(),
            TimelineEventKind::YellowCard.style(),
        ),
        Span::styled(") / (".to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(
            away_cards.red.to_string(),
            TimelineEventKind::RedCard.style(),
        ),
        Span::raw("-"),
        Span::styled(
            away_cards.yellow.to_string(),
            TimelineEventKind::YellowCard.style(),
        ),
        Span::styled(")".to_string(), Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{:spacer_width$}", "")),
        Span::styled(context, Style::default().fg(Color::DarkGray)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(time_label, Style::default().fg(Color::DarkGray)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(status_label, match_detail_status_style(match_.status)),
    ])
}

fn match_detail_status_style(status: crate::domain::MatchStatus) -> Style {
    match status {
        crate::domain::MatchStatus::Live => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
        _ => Style::default().fg(Color::DarkGray),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CardTotals {
    red: usize,
    yellow: usize,
}

fn card_totals_for_team(
    events: &[&crate::domain::TimelineEvent],
    team_id: Option<&crate::domain::TeamId>,
) -> CardTotals {
    let Some(team_id) = team_id else {
        return CardTotals::default();
    };

    let mut totals = CardTotals::default();

    for event in events {
        if event.team_id.as_ref() != Some(team_id) {
            continue;
        }

        match timeline_event_kind(event) {
            Some(TimelineEventKind::YellowCard) => totals.yellow += 1,
            Some(TimelineEventKind::RedCard) => totals.red += 1,
            _ => {}
        }
    }

    totals
}

fn time_cell(value: &str, width: usize, selected: bool) -> String {
    let value = if selected {
        format!(">{}", truncate(value, width.saturating_sub(1)))
    } else {
        truncate(value, width)
    };

    format!("{value:<width$}")
}

fn timeline_event_lines(
    match_: &crate::domain::Match,
    event: &crate::domain::TimelineEvent,
    content_width: usize,
) -> Vec<Line<'static>> {
    let kind = timeline_event_kind(event).unwrap_or(TimelineEventKind::Key);
    if kind == TimelineEventKind::HalfTime {
        return vec![timeline_marker_line(content_width, " HALF TIME ")];
    }

    let minute = event.minute.as_deref().unwrap_or("--");
    let description = event
        .description
        .as_deref()
        .map(clean_timeline_description)
        .unwrap_or_else(|| kind.label().to_string());
    let event_parts = timeline_event_parts(kind, &description);
    let axis_style = Style::default().fg(Color::DarkGray);
    let minute_style = Style::default().fg(Color::White);
    let side_width = timeline_side_width(content_width);
    let left_blank_width = side_width + TIMELINE_MINUTE_BAND_WIDTH;
    let right_blank_width = TIMELINE_MINUTE_BAND_WIDTH + side_width;
    let side_lines = timeline_side_lines(
        event_parts,
        side_width,
        timeline_event_alignment(match_, event, kind),
    );

    side_lines
        .into_iter()
        .enumerate()
        .map(|(index, side_spans)| {
            let minute_band = if index == 0 {
                format!(" {minute:>TIMELINE_MINUTE_WIDTH$} ")
            } else {
                " ".repeat(TIMELINE_MINUTE_BAND_WIDTH)
            };

            match timeline_event_side(match_, event, kind) {
                TimelineSide::Home => {
                    let mut spans = side_spans;
                    spans.extend([
                        Span::styled(minute_band, minute_style),
                        Span::styled("|", axis_style),
                        Span::raw(format!("{:<right_blank_width$}", "")),
                    ]);
                    Line::from(spans)
                }
                TimelineSide::Away | TimelineSide::Neutral => {
                    let mut spans = vec![
                        Span::raw(format!("{:>left_blank_width$}", "")),
                        Span::styled("|", axis_style),
                        Span::styled(minute_band, minute_style),
                    ];
                    spans.extend(side_spans);
                    Line::from(spans)
                }
            }
        })
        .collect()
}

fn timeline_spine_line(content_width: usize) -> Line<'static> {
    let side_width = timeline_side_width(content_width);
    let left_blank_width = side_width + TIMELINE_MINUTE_BAND_WIDTH;

    Line::from(vec![
        Span::raw(format!("{:>left_blank_width$}", "")),
        Span::styled("|", Style::default().fg(Color::DarkGray)),
    ])
}

fn timeline_marker_line(content_width: usize, label: &str) -> Line<'static> {
    let width = content_width.max(label.chars().count());
    let label_width = label.chars().count();
    let left_width = width.saturating_sub(label_width) / 2;
    let right_width = width.saturating_sub(label_width + left_width);

    Line::from(vec![
        Span::styled("-".repeat(left_width), Style::default().fg(Color::DarkGray)),
        Span::styled(label.to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(
            "-".repeat(right_width),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}

fn timeline_side_width(content_width: usize) -> usize {
    let fixed_width = (TIMELINE_MINUTE_BAND_WIDTH * 2) + 1;
    (content_width.saturating_sub(fixed_width) / 2).max(8)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimelineAlignment {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimelineSide {
    Home,
    Away,
    Neutral,
}

fn timeline_event_side(
    match_: &crate::domain::Match,
    event: &crate::domain::TimelineEvent,
    kind: TimelineEventKind,
) -> TimelineSide {
    let is_home_event = event.team_id.as_ref() == match_.home_team_id.as_ref();
    let is_away_event = event.team_id.as_ref() == match_.away_team_id.as_ref();

    match (
        kind == TimelineEventKind::OwnGoal,
        is_home_event,
        is_away_event,
    ) {
        (true, true, _) => TimelineSide::Away,
        (true, _, true) => TimelineSide::Home,
        (_, true, _) => TimelineSide::Home,
        (_, _, true) => TimelineSide::Away,
        _ => TimelineSide::Neutral,
    }
}

fn timeline_event_alignment(
    match_: &crate::domain::Match,
    event: &crate::domain::TimelineEvent,
    kind: TimelineEventKind,
) -> TimelineAlignment {
    match timeline_event_side(match_, event, kind) {
        TimelineSide::Home => TimelineAlignment::Left,
        TimelineSide::Away | TimelineSide::Neutral => TimelineAlignment::Right,
    }
}

fn timeline_side_lines(
    parts: Vec<Span<'static>>,
    width: usize,
    alignment: TimelineAlignment,
) -> Vec<Vec<Span<'static>>> {
    wrap_spans(parts, width)
        .into_iter()
        .map(|parts| timeline_side_spans(parts, width, alignment))
        .collect()
}

fn timeline_side_spans(
    parts: Vec<Span<'static>>,
    width: usize,
    alignment: TimelineAlignment,
) -> Vec<Span<'static>> {
    let text_width = spans_width(&parts);
    let padding = width.saturating_sub(text_width);
    let mut spans = Vec::new();

    if alignment == TimelineAlignment::Right {
        spans.push(Span::raw(format!("{:padding$}", "")));
    }

    spans.extend(parts);

    if alignment == TimelineAlignment::Left {
        spans.push(Span::raw(format!("{:padding$}", "")));
    }

    spans
}

fn wrap_spans(parts: Vec<Span<'static>>, max_width: usize) -> Vec<Vec<Span<'static>>> {
    let max_width = max_width.max(1);
    let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut line_width = 0;

    for part in parts {
        let style = part.style;
        if part.content == "\n" {
            if !lines.last().is_some_and(Vec::is_empty) {
                lines.push(Vec::new());
                line_width = 0;
            }
            continue;
        }

        let tokens = wrap_tokens(&part.content);

        for token in tokens {
            if token.trim().is_empty() && line_width == 0 {
                continue;
            }

            for chunk in split_token_to_width(&token, max_width) {
                let chunk_width = chunk.chars().count();
                if line_width > 0 && line_width + chunk_width > max_width {
                    lines.push(Vec::new());
                    line_width = 0;
                }

                if chunk.trim().is_empty() && line_width == 0 {
                    continue;
                }

                line_width += chunk_width;
                lines
                    .last_mut()
                    .expect("timeline wrap has at least one line")
                    .push(Span::styled(chunk, style));
            }
        }
    }

    if lines.last().is_some_and(Vec::is_empty) && lines.len() > 1 {
        lines.pop();
    }

    if lines.is_empty() || lines[0].is_empty() {
        vec![vec![Span::raw("")]]
    } else {
        lines
    }
}

fn wrap_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_is_whitespace = None;

    for character in value.chars() {
        let is_whitespace = character.is_whitespace();
        if current_is_whitespace.is_some_and(|state| state != is_whitespace) {
            tokens.push(std::mem::take(&mut current));
        }
        current_is_whitespace = Some(is_whitespace);
        current.push(character);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn split_token_to_width(token: &str, max_width: usize) -> Vec<String> {
    if token.chars().count() <= max_width {
        return vec![token.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for character in token.chars() {
        if current.chars().count() == max_width {
            chunks.push(std::mem::take(&mut current));
        }
        current.push(character);
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn spans_width(parts: &[Span<'static>]) -> usize {
    parts.iter().map(|part| part.content.chars().count()).sum()
}

fn match_day_color(index: usize) -> Color {
    if index % 2 == 0 {
        Color::Cyan
    } else {
        Color::Magenta
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TimelineEventKind {
    Goal,
    OwnGoal,
    YellowCard,
    RedCard,
    Substitution,
    HalfTime,
    Key,
}

impl TimelineEventKind {
    fn label(self) -> &'static str {
        match self {
            Self::Goal => "Goal",
            Self::OwnGoal => "Own goal",
            Self::YellowCard => "Yellow card",
            Self::RedCard => "Red card",
            Self::Substitution => "Substitution",
            Self::HalfTime => "Half time",
            Self::Key => "Key event",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Goal | Self::OwnGoal => Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            Self::YellowCard => Style::default().fg(Color::Yellow),
            Self::RedCard => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            Self::Substitution => Style::default().fg(Color::Cyan),
            Self::HalfTime => Style::default().fg(Color::DarkGray),
            Self::Key => Style::default().fg(Color::White),
        }
    }
}

fn timeline_event_kind(event: &crate::domain::TimelineEvent) -> Option<TimelineEventKind> {
    let description = event
        .description
        .as_deref()
        .map(clean_timeline_description)
        .unwrap_or_default();
    let normalized = description.to_ascii_lowercase();
    if timeline_event_is_half_time(&normalized) {
        return Some(TimelineEventKind::HalfTime);
    }
    if timeline_event_is_noise(&normalized) {
        return None;
    }
    if normalized.contains("own goal") {
        return Some(TimelineEventKind::OwnGoal);
    }

    match event.event_type {
        Some(0) => Some(TimelineEventKind::Goal),
        Some(2) => Some(TimelineEventKind::YellowCard),
        Some(3) => Some(TimelineEventKind::RedCard),
        Some(5) => Some(TimelineEventKind::Substitution),
        Some(7 | 8 | 26 | 71 | 78 | 83) => Some(TimelineEventKind::Key),
        _ => None,
    }
}

fn timeline_event_is_noise(description: &str) -> bool {
    description.contains("hydration")
        || description.contains("drinks break")
        || description.contains("cooling break")
        || description.contains("paused")
        || description.contains("pause in play")
        || description.contains("play is paused")
        || description.contains("resumed")
        || description.contains("resume")
        || description.contains("red card given")
        || description.contains("goal awarded")
        || description.contains("goal disallowed")
        || description.contains("before the second half begins")
        || description.contains("referee signals")
        || description.contains("referee brings")
}

fn timeline_event_is_half_time(description: &str) -> bool {
    description.contains("half time")
        || description.contains("half-time")
        || description.contains("end first half")
        || description.contains("end of first half")
        || (description.contains("first")
            && description.contains("brings")
            && description.contains("end"))
}

fn timeline_event_matches_filter(
    event: &crate::domain::TimelineEvent,
    filter: TimelineFilter,
) -> bool {
    match (filter, timeline_event_kind(event)) {
        (TimelineFilter::All, Some(_)) => true,
        (TimelineFilter::Goals, Some(TimelineEventKind::HalfTime)) => true,
        (TimelineFilter::Goals, Some(TimelineEventKind::Goal)) => true,
        (TimelineFilter::Goals, Some(TimelineEventKind::OwnGoal)) => true,
        (TimelineFilter::RedCards, Some(TimelineEventKind::RedCard)) => true,
        (TimelineFilter::YellowCards, Some(TimelineEventKind::YellowCard)) => true,
        (TimelineFilter::Substitutions, Some(TimelineEventKind::Substitution)) => true,
        _ => false,
    }
}

fn timeline_event_matches_filters(
    event: &crate::domain::TimelineEvent,
    filters: &[TimelineFilter],
) -> bool {
    if filters.is_empty() {
        return timeline_event_matches_filter(event, TimelineFilter::All);
    }

    filters
        .iter()
        .any(|filter| timeline_event_matches_filter(event, *filter))
}

fn clean_timeline_description(value: &str) -> String {
    value.replace('\n', " ").trim().to_string()
}

fn timeline_event_parts(kind: TimelineEventKind, description: &str) -> Vec<Span<'static>> {
    match kind {
        TimelineEventKind::Goal => vec![
            Span::styled("GOAL ".to_string(), kind.style()),
            Span::styled(primary_name(description), kind.style()),
        ],
        TimelineEventKind::OwnGoal => vec![
            Span::styled("GOAL ".to_string(), kind.style()),
            Span::styled(format!("{} (OG)", primary_name(description)), kind.style()),
        ],
        TimelineEventKind::YellowCard | TimelineEventKind::RedCard => {
            vec![Span::styled(primary_name(description), kind.style())]
        }
        TimelineEventKind::Substitution => substitution_parts(description),
        TimelineEventKind::HalfTime | TimelineEventKind::Key => {
            vec![Span::styled(description.to_string(), kind.style())]
        }
    }
}

fn substitution_parts(description: &str) -> Vec<Span<'static>> {
    let (player_in, player_out) = substitution_names(description);

    vec![
        Span::styled("-> ".to_string(), Style::default().fg(Color::Green)),
        Span::styled(player_in, Style::default().fg(Color::Cyan)),
        Span::raw("\n"),
        Span::styled("<- ".to_string(), Style::default().fg(Color::Red)),
        Span::styled(player_out, Style::default().fg(Color::Cyan)),
    ]
}

fn substitution_names(description: &str) -> (String, String) {
    let description = strip_prefix_case_insensitive(description, "substitution:")
        .trim()
        .trim_end_matches('.');

    if let Some((player_in, rest)) =
        split_once_case_insensitive(description, " (in) comes off the bench to replace ")
    {
        let player_out = split_once_case_insensitive(rest, " (out)")
            .map(|(name, _)| name)
            .unwrap_or(rest);
        return (primary_name(player_in), primary_name(player_out));
    }

    if let Some((player_in, player_out)) = split_once_case_insensitive(description, " replaces ") {
        return (primary_name(player_in), primary_name(player_out));
    }

    if let Some((player_out, player_in)) = split_once_case_insensitive(description, " replaced by ")
    {
        return (primary_name(player_in), primary_name(player_out));
    }

    if let Some((player_in, player_out)) =
        split_once_case_insensitive(description, " comes on for ")
    {
        return (primary_name(player_in), primary_name(player_out));
    }

    if let Some((player_in, player_out)) =
        split_once_case_insensitive(description, " enters the game and replaces ")
    {
        return (primary_name(player_in), primary_name(player_out));
    }

    (primary_name(description), "unknown".to_string())
}

fn primary_name(description: &str) -> String {
    let trimmed = description.trim().trim_end_matches('.');
    let cut_points = [
        " (",
        " is ",
        " was ",
        " has ",
        " receives ",
        " gets ",
        " shown ",
        " commits ",
        " scores",
        " converts",
        " replaces ",
        " comes on",
    ];
    let cut_index = cut_points
        .iter()
        .filter_map(|marker| trimmed.to_ascii_lowercase().find(marker).map(|index| index))
        .min()
        .unwrap_or_else(|| trimmed.len());

    trimmed[..cut_index].trim().to_string()
}

fn strip_prefix_case_insensitive<'a>(value: &'a str, prefix: &str) -> &'a str {
    if value
        .get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    {
        &value[prefix.len()..]
    } else {
        value
    }
}

fn split_once_case_insensitive<'a>(value: &'a str, delimiter: &str) -> Option<(&'a str, &'a str)> {
    value
        .to_ascii_lowercase()
        .find(delimiter)
        .map(|index| (&value[..index], &value[index + delimiter.len()..]))
}

fn match_context(match_: &crate::domain::Match) -> String {
    match_
        .group_name
        .clone()
        .unwrap_or_else(|| match_.stage_name.clone())
}

fn score_label(match_: &crate::domain::Match) -> String {
    match (match_.home_score, match_.away_score) {
        (Some(home), Some(away)) => format!("{home}-{away}"),
        _ => "vs".to_string(),
    }
}

fn match_home_span<'a>(app: &App, match_: &crate::domain::Match, selected: bool) -> Span<'a> {
    Span::styled(
        format!(
            "{:<4}",
            app.match_team_code(match_.home_team_id.as_ref(), &match_.home_team_name)
        ),
        selected_style(
            team_style(winning_side(match_) == Some(MatchSide::Home)),
            selected,
        ),
    )
}

fn match_away_span<'a>(app: &App, match_: &crate::domain::Match, selected: bool) -> Span<'a> {
    Span::styled(
        format!(
            "{:<4}",
            app.match_team_code(match_.away_team_id.as_ref(), &match_.away_team_name)
        ),
        selected_style(
            team_style(winning_side(match_) == Some(MatchSide::Away)),
            selected,
        ),
    )
}

fn selected_style(style: Style, selected: bool) -> Style {
    if selected {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

fn team_style(winning: bool) -> Style {
    if winning {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    } else {
        Style::default().fg(Color::Gray)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MatchSide {
    Home,
    Away,
}

fn winning_side(match_: &crate::domain::Match) -> Option<MatchSide> {
    if match_
        .winner_team_id
        .as_ref()
        .is_some_and(|winner| Some(winner) == match_.home_team_id.as_ref())
    {
        return Some(MatchSide::Home);
    }
    if match_
        .winner_team_id
        .as_ref()
        .is_some_and(|winner| Some(winner) == match_.away_team_id.as_ref())
    {
        return Some(MatchSide::Away);
    }

    match (match_.home_score, match_.away_score) {
        (Some(home), Some(away)) if home > away => Some(MatchSide::Home),
        (Some(home), Some(away)) if away > home => Some(MatchSide::Away),
        _ => match (match_.home_penalty_score, match_.away_penalty_score) {
            (Some(home), Some(away)) if home > away => Some(MatchSide::Home),
            (Some(home), Some(away)) if away > home => Some(MatchSide::Away),
            _ => None,
        },
    }
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
    use crate::domain::{MatchId, TimelineEvent};

    #[test]
    fn timeline_filters_var_goal_review_noise() {
        assert!(!super::timeline_event_matches_filter(
            &event("Goal awarded following VAR Review."),
            crate::app::TimelineFilter::All
        ));
        assert!(!super::timeline_event_matches_filter(
            &event("Goal disallowed following VAR Review."),
            crate::app::TimelineFilter::All
        ));
    }

    fn event(description: &'static str) -> TimelineEvent {
        TimelineEvent {
            match_id: MatchId::from("4001"),
            event_index: 1,
            event_type: Some(7),
            team_id: None,
            player_id: None,
            minute: Some("77'".to_string()),
            description: Some(description.to_string()),
        }
    }
}
