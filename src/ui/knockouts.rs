use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::{
    app::{App, KnockoutRoundFilter},
    domain::{Group, Match, QualificationState, TeamId},
    ui::components,
};

const CARD_WIDTH: usize = 25;
const COLUMN_GAP: usize = 2;
const META_WIDTH: usize = CARD_WIDTH - 2;
const DATE_TIME_WIDTH: usize = 5;
const TEAM_CONTENT_WIDTH: usize = 8;
const SCORE_CONTENT_WIDTH: usize = 2;

const FINAL_ORDER: [u16; 1] = [104];
const THIRD_ORDER: [u16; 1] = [103];
const LEFT_ROUND32_ORDER: [u16; 8] = [74, 77, 73, 75, 83, 84, 81, 82];
const RIGHT_ROUND32_ORDER: [u16; 8] = [76, 78, 79, 80, 86, 88, 85, 87];
const LEFT_ROUND16_ORDER: [u16; 4] = [89, 90, 93, 94];
const RIGHT_ROUND16_ORDER: [u16; 4] = [91, 92, 95, 96];
const LEFT_QUARTER_ORDER: [u16; 2] = [97, 98];
const RIGHT_QUARTER_ORDER: [u16; 2] = [99, 100];
const LEFT_SEMI_ORDER: [u16; 1] = [101];
const RIGHT_SEMI_ORDER: [u16; 1] = [102];
const CENTER_ORDER: [u16; 2] = [104, 103];

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let rounds = app.visible_knockout_rounds();
    let header = vec![
        components::filter_line(app.knockout_round_label()),
        components::label_value_line(
            "Rows: ",
            format!("{} knock-out matches loaded", app.knockout_match_count()),
        ),
        components::shortcut_menu_line_for_scope(
            "Rounds: ",
            ["[a]ll", "[r]ound of 32", "r[o]und of 16"],
            components::screen_scope_active(app),
        ),
        components::shortcut_menu_line_for_scope(
            "        ",
            ["q[u]arters", "[s]emis", "[t]hird", "[f]inal"],
            components::screen_scope_active(app),
        ),
        Line::from(""),
        Line::from("W/RU placeholders resolve once upstream matches have winners."),
        Line::from("Best-third slots stay as 3ABCDE-style labels until FIFA assigns teams."),
        Line::from("Tab focuses the next round column; Up/Down scrolls content."),
    ];

    components::render_screen_frame_unwrapped_scrolled_with_sticky(
        frame,
        area,
        "Knock-outs",
        header,
        bracket_sticky_lines(&rounds),
        bracket_body_lines(app, &rounds),
        components::content_focused(app),
        app.content_scroll(),
        app.content_horizontal_scroll(),
    )
}

fn bracket_lines(app: &App, rounds: &[KnockoutRoundFilter]) -> Vec<Line<'static>> {
    let mut lines = bracket_sticky_lines(rounds);
    lines.extend(bracket_body_lines(app, rounds));
    lines
}

fn bracket_sticky_lines(rounds: &[KnockoutRoundFilter]) -> Vec<Line<'static>> {
    let columns = mirrored_columns(rounds);
    let mut rows = vec![StyledRow::default(); 1];

    for (column, spec) in columns.iter().enumerate() {
        let x = column * (CARD_WIDTH + COLUMN_GAP);
        put_line(
            &mut rows,
            0,
            x,
            StyledLine::styled(center(spec.title, CARD_WIDTH), column_title_style()),
        );
    }

    rows.into_iter().map(Line::from).collect()
}

fn bracket_body_lines(app: &App, rounds: &[KnockoutRoundFilter]) -> Vec<Line<'static>> {
    let columns = mirrored_columns(rounds);
    let mut rows = vec![StyledRow::default(); mirrored_bracket_body_height(&columns)];

    for (column, spec) in columns.iter().enumerate() {
        let x = column * (CARD_WIDTH + COLUMN_GAP);

        for match_number in spec.matches.iter().copied() {
            let y = mirrored_match_y(spec.kind, match_number).saturating_sub(1);
            draw_match_card(&mut rows, y, x, app, match_def(match_number));
        }
    }

    rows.into_iter().map(Line::from).collect()
}

fn mirrored_bracket_body_height(columns: &[ColumnSpec]) -> usize {
    columns
        .iter()
        .flat_map(|column| {
            column
                .matches
                .iter()
                .map(|match_number| mirrored_match_y(column.kind, *match_number) + 2)
        })
        .max()
        .unwrap_or(3)
}

fn draw_match_card(rows: &mut [StyledRow], y: usize, x: usize, app: &App, def: MatchDef) {
    let _round = def.round;
    let match_ = app.match_by_number(def.number);
    let home = resolve_participant(app, match_, Side::Home, def.home);
    let away = resolve_participant(app, match_, Side::Away, def.away);
    let meta = format!("M{}", def.number);
    let (date, time) = match_
        .map(date_time_labels)
        .unwrap_or_else(|| ("--/--".to_string(), "--:--".to_string()));
    let meta = truncate(&meta, META_WIDTH);
    let border_style = match_border_style(def);

    put_line(rows, y, x, meta_line(&meta, border_style));
    put_line(rows, y + 1, x, participant_line(&date, &home, border_style));
    put_line(rows, y + 2, x, participant_line(&time, &away, border_style));
}

fn meta_line(meta: &str, border_style: Style) -> StyledLine {
    let mut line = StyledLine::default();
    line.push_styled("|", border_style);
    line.push_styled(center(meta, META_WIDTH), meta_style());
    line.push_styled("|", border_style);
    line
}

fn participant_line(
    time_label: &str,
    participant: &Participant,
    border_style: Style,
) -> StyledLine {
    let mut line = StyledLine::default();
    let team_code = truncate(&participant.code, TEAM_CONTENT_WIDTH);
    let score_label = truncate(
        &participant
            .score
            .map(|score| score.to_string())
            .unwrap_or_else(|| "-".to_string()),
        SCORE_CONTENT_WIDTH,
    );
    let team_padding = TEAM_CONTENT_WIDTH.saturating_sub(team_code.chars().count());
    let score_padding = SCORE_CONTENT_WIDTH.saturating_sub(score_label.chars().count());
    let participant_style = participant.style();
    let score_style = if participant.score.is_some() {
        participant_style
    } else {
        Style::default().fg(Color::DarkGray)
    };

    line.push_styled("| ", border_style);
    line.push_styled(
        format!(
            "{:<DATE_TIME_WIDTH$}",
            truncate(time_label, DATE_TIME_WIDTH)
        ),
        meta_style(),
    );
    line.push_styled(" | ", border_style);
    line.push_raw(" ".repeat(team_padding));
    line.push_styled(team_code, participant_style);
    line.push_styled(" | ", border_style);
    line.push_raw(" ".repeat(score_padding));
    line.push_styled(score_label, score_style);
    line.push_styled(" |", border_style);
    line
}

fn resolve_participant(
    app: &App,
    match_: Option<&Match>,
    side: Side,
    source: ParticipantSource,
) -> Participant {
    if let Some((team_id, name)) = match_.and_then(|match_| match_side_team(match_, side)) {
        return Participant {
            code: app.match_team_code(Some(team_id), name),
            score: match_.and_then(|match_| match_side_score(match_, side)),
            team_id: Some(team_id.clone()),
            qualification_state: QualificationState::Qualified,
            winner: match_.is_some_and(|match_| side_won(match_, side, team_id)),
            favorite: app.team_is_favorite(team_id),
        };
    }

    let mut participant = match source {
        ParticipantSource::GroupPosition { group, position } => app
            .standing_for_group_position(group, position)
            .map(|row| Participant {
                code: format!(
                    "{position}{}-{}",
                    group.letter(),
                    app.match_team_code(Some(&row.team_id), &row.team_name)
                ),
                score: None,
                team_id: Some(row.team_id.clone()),
                qualification_state: row.qualification_state(),
                winner: false,
                favorite: app.team_is_favorite(&row.team_id),
            })
            .unwrap_or_else(|| Participant::placeholder(format!("{position}{}", group.letter()))),
        ParticipantSource::BestThird(groups) => {
            Participant::placeholder(format!("3{}", group_letters(groups)))
        }
        ParticipantSource::Winner(match_number) => resolve_upstream_result(
            app,
            match_number,
            UpstreamResult::Winner,
            format!("W{match_number}"),
        ),
        ParticipantSource::RunnerUp(match_number) => resolve_upstream_result(
            app,
            match_number,
            UpstreamResult::RunnerUp,
            format!("RU{match_number}"),
        ),
    };

    participant.winner = false;
    participant
}

fn resolve_upstream_result(
    app: &App,
    match_number: u16,
    result: UpstreamResult,
    placeholder: String,
) -> Participant {
    let Some(match_) = app.match_by_number(match_number) else {
        return Participant::placeholder(placeholder);
    };
    let Some(winner_id) = match_.winner_team_id.as_ref() else {
        return Participant::placeholder(placeholder);
    };
    let Some((team_id, name)) = (match result {
        UpstreamResult::Winner => [Side::Home, Side::Away]
            .into_iter()
            .filter_map(|side| match_side_team(match_, side))
            .find(|(team_id, _)| *team_id == winner_id),
        UpstreamResult::RunnerUp => [Side::Home, Side::Away]
            .into_iter()
            .filter_map(|side| match_side_team(match_, side))
            .find(|(team_id, _)| *team_id != winner_id),
    }) else {
        return Participant::placeholder(placeholder);
    };

    Participant {
        code: app.match_team_code(Some(team_id), name),
        score: None,
        team_id: Some(team_id.clone()),
        qualification_state: QualificationState::Qualified,
        winner: false,
        favorite: app.team_is_favorite(team_id),
    }
}

fn match_side_team(match_: &Match, side: Side) -> Option<(&TeamId, &str)> {
    match side {
        Side::Home => match_
            .home_team_id
            .as_ref()
            .map(|team_id| (team_id, match_.home_team_name.as_str())),
        Side::Away => match_
            .away_team_id
            .as_ref()
            .map(|team_id| (team_id, match_.away_team_name.as_str())),
    }
}

fn match_side_score(match_: &Match, side: Side) -> Option<u8> {
    match side {
        Side::Home => match_.home_score,
        Side::Away => match_.away_score,
    }
}

fn side_won(match_: &Match, side: Side, team_id: &TeamId) -> bool {
    if match_
        .winner_team_id
        .as_ref()
        .is_some_and(|winner_id| winner_id == team_id)
    {
        return true;
    }

    let Some(home_score) = match_.home_score else {
        return false;
    };
    let Some(away_score) = match_.away_score else {
        return false;
    };

    match side {
        Side::Home => {
            home_score > away_score
                || (home_score == away_score
                    && match_
                        .home_penalty_score
                        .zip(match_.away_penalty_score)
                        .is_some_and(|(home_penalty_score, away_penalty_score)| {
                            home_penalty_score > away_penalty_score
                        }))
        }
        Side::Away => {
            away_score > home_score
                || (home_score == away_score
                    && match_
                        .home_penalty_score
                        .zip(match_.away_penalty_score)
                        .is_some_and(|(home_penalty_score, away_penalty_score)| {
                            away_penalty_score > home_penalty_score
                        }))
        }
    }
}

fn date_time_labels(match_: &Match) -> (String, String) {
    let local = match_.utc_start.to_zoned(jiff::tz::TimeZone::system());
    (
        local.strftime("%d/%m").to_string(),
        local.strftime("%H:%M").to_string(),
    )
}

fn column_title_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

fn border_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn match_border_style(def: MatchDef) -> Style {
    match def.round {
        KnockoutRoundFilter::Final => Style::default().fg(Color::Yellow),
        KnockoutRoundFilter::ThirdPlace => Style::default().fg(Color::Indexed(208)),
        _ => border_style(),
    }
}

fn meta_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

#[derive(Clone, Debug)]
struct Participant {
    code: String,
    score: Option<u8>,
    team_id: Option<TeamId>,
    qualification_state: QualificationState,
    winner: bool,
    favorite: bool,
}

impl Participant {
    fn placeholder(label: String) -> Self {
        Self {
            code: label,
            score: None,
            team_id: None,
            qualification_state: QualificationState::Open,
            winner: false,
            favorite: false,
        }
    }

    fn style(&self) -> Style {
        let mut style = if self.winner {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else if self.team_id.is_some() && self.qualification_state.is_disqualified() {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if self.team_id.is_some() && !self.qualification_state.is_qualified() {
            Style::default().fg(Color::DarkGray)
        } else if self.team_id.is_some() {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        if self.favorite {
            style = style.add_modifier(Modifier::UNDERLINED);
        }

        style
    }
}

#[derive(Clone, Copy)]
enum Side {
    Home,
    Away,
}

#[derive(Clone, Copy)]
enum UpstreamResult {
    Winner,
    RunnerUp,
}

#[derive(Clone, Copy)]
enum ColumnKind {
    RoundOf32,
    RoundOf16,
    QuarterFinal,
    SemiFinal,
    Center,
}

struct ColumnSpec {
    title: &'static str,
    kind: ColumnKind,
    matches: &'static [u16],
}

fn mirrored_columns(rounds: &[KnockoutRoundFilter]) -> Vec<ColumnSpec> {
    let mut columns = Vec::new();

    if rounds.contains(&KnockoutRoundFilter::RoundOf32) {
        columns.push(ColumnSpec {
            title: "Round of 32",
            kind: ColumnKind::RoundOf32,
            matches: &LEFT_ROUND32_ORDER,
        });
    }
    if rounds.contains(&KnockoutRoundFilter::RoundOf16) {
        columns.push(ColumnSpec {
            title: "Round of 16",
            kind: ColumnKind::RoundOf16,
            matches: &LEFT_ROUND16_ORDER,
        });
    }
    if rounds.contains(&KnockoutRoundFilter::QuarterFinal) {
        columns.push(ColumnSpec {
            title: "Quarter-finals",
            kind: ColumnKind::QuarterFinal,
            matches: &LEFT_QUARTER_ORDER,
        });
    }
    if rounds.contains(&KnockoutRoundFilter::SemiFinal) {
        columns.push(ColumnSpec {
            title: "Semi-finals",
            kind: ColumnKind::SemiFinal,
            matches: &LEFT_SEMI_ORDER,
        });
    }

    let mut center_matches = Vec::new();
    if rounds.contains(&KnockoutRoundFilter::Final) {
        center_matches.push(104);
    }
    if rounds.contains(&KnockoutRoundFilter::ThirdPlace) {
        center_matches.push(103);
    }
    if !center_matches.is_empty() {
        let matches: &'static [u16] = if center_matches == CENTER_ORDER {
            &CENTER_ORDER
        } else if center_matches == [104] {
            &FINAL_ORDER
        } else {
            &THIRD_ORDER
        };
        columns.push(ColumnSpec {
            title: "Final / Third",
            kind: ColumnKind::Center,
            matches,
        });
    }

    if rounds.contains(&KnockoutRoundFilter::SemiFinal) {
        columns.push(ColumnSpec {
            title: "Semi-finals",
            kind: ColumnKind::SemiFinal,
            matches: &RIGHT_SEMI_ORDER,
        });
    }
    if rounds.contains(&KnockoutRoundFilter::QuarterFinal) {
        columns.push(ColumnSpec {
            title: "Quarter-finals",
            kind: ColumnKind::QuarterFinal,
            matches: &RIGHT_QUARTER_ORDER,
        });
    }
    if rounds.contains(&KnockoutRoundFilter::RoundOf16) {
        columns.push(ColumnSpec {
            title: "Round of 16",
            kind: ColumnKind::RoundOf16,
            matches: &RIGHT_ROUND16_ORDER,
        });
    }
    if rounds.contains(&KnockoutRoundFilter::RoundOf32) {
        columns.push(ColumnSpec {
            title: "Round of 32",
            kind: ColumnKind::RoundOf32,
            matches: &RIGHT_ROUND32_ORDER,
        });
    }

    columns
}

fn mirrored_match_y(kind: ColumnKind, match_number: u16) -> usize {
    match kind {
        ColumnKind::RoundOf32 => {
            match_position(match_number, &LEFT_ROUND32_ORDER, &RIGHT_ROUND32_ORDER).unwrap_or(0) * 4
                + 1
        }
        ColumnKind::RoundOf16 => {
            match_position(match_number, &LEFT_ROUND16_ORDER, &RIGHT_ROUND16_ORDER).unwrap_or(0) * 8
                + 3
        }
        ColumnKind::QuarterFinal => {
            match_position(match_number, &LEFT_QUARTER_ORDER, &RIGHT_QUARTER_ORDER).unwrap_or(0)
                * 16
                + 7
        }
        ColumnKind::SemiFinal => 15,
        ColumnKind::Center => match match_number {
            104 => 13,
            103 => 21,
            _ => 17,
        },
    }
}

fn match_position(match_number: u16, left: &[u16], right: &[u16]) -> Option<usize> {
    left.iter()
        .position(|number| *number == match_number)
        .or_else(|| right.iter().position(|number| *number == match_number))
}

#[derive(Clone, Copy)]
struct MatchDef {
    number: u16,
    round: KnockoutRoundFilter,
    home: ParticipantSource,
    away: ParticipantSource,
}

#[derive(Clone, Copy)]
enum ParticipantSource {
    GroupPosition { group: Group, position: u8 },
    BestThird(&'static [Group]),
    Winner(u16),
    RunnerUp(u16),
}

fn match_def(number: u16) -> MatchDef {
    use Group::*;
    use KnockoutRoundFilter::*;
    use ParticipantSource::*;

    match number {
        73 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: A,
                position: 2,
            },
            away: GroupPosition {
                group: B,
                position: 2,
            },
        },
        74 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: E,
                position: 1,
            },
            away: BestThird(&[A, B, C, D, F]),
        },
        75 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: F,
                position: 1,
            },
            away: GroupPosition {
                group: C,
                position: 2,
            },
        },
        76 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: C,
                position: 1,
            },
            away: GroupPosition {
                group: F,
                position: 2,
            },
        },
        77 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: I,
                position: 1,
            },
            away: BestThird(&[C, D, F, G, H]),
        },
        78 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: E,
                position: 2,
            },
            away: GroupPosition {
                group: I,
                position: 2,
            },
        },
        79 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: A,
                position: 1,
            },
            away: BestThird(&[C, E, F, H, I]),
        },
        80 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: L,
                position: 1,
            },
            away: BestThird(&[E, H, I, J, K]),
        },
        81 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: D,
                position: 1,
            },
            away: BestThird(&[B, E, F, I, J]),
        },
        82 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: G,
                position: 1,
            },
            away: BestThird(&[A, E, H, I, J]),
        },
        83 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: K,
                position: 2,
            },
            away: GroupPosition {
                group: L,
                position: 2,
            },
        },
        84 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: H,
                position: 1,
            },
            away: GroupPosition {
                group: J,
                position: 2,
            },
        },
        85 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: B,
                position: 1,
            },
            away: BestThird(&[E, F, G, I, J]),
        },
        86 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: J,
                position: 1,
            },
            away: GroupPosition {
                group: H,
                position: 2,
            },
        },
        87 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: K,
                position: 1,
            },
            away: BestThird(&[D, E, I, J, L]),
        },
        88 => MatchDef {
            number,
            round: RoundOf32,
            home: GroupPosition {
                group: D,
                position: 2,
            },
            away: GroupPosition {
                group: G,
                position: 2,
            },
        },
        89 => linked_match(number, RoundOf16, 74, 77),
        90 => linked_match(number, RoundOf16, 73, 75),
        91 => linked_match(number, RoundOf16, 76, 78),
        92 => linked_match(number, RoundOf16, 79, 80),
        93 => linked_match(number, RoundOf16, 83, 84),
        94 => linked_match(number, RoundOf16, 81, 82),
        95 => linked_match(number, RoundOf16, 86, 88),
        96 => linked_match(number, RoundOf16, 85, 87),
        97 => linked_match(number, QuarterFinal, 89, 90),
        98 => linked_match(number, QuarterFinal, 93, 94),
        99 => linked_match(number, QuarterFinal, 91, 92),
        100 => linked_match(number, QuarterFinal, 95, 96),
        101 => linked_match(number, SemiFinal, 97, 98),
        102 => linked_match(number, SemiFinal, 99, 100),
        103 => MatchDef {
            number,
            round: ThirdPlace,
            home: RunnerUp(101),
            away: RunnerUp(102),
        },
        104 => linked_match(number, Final, 101, 102),
        _ => panic!("unknown knockout match number {number}"),
    }
}

fn linked_match(
    number: u16,
    round: KnockoutRoundFilter,
    home_winner: u16,
    away_winner: u16,
) -> MatchDef {
    MatchDef {
        number,
        round,
        home: ParticipantSource::Winner(home_winner),
        away: ParticipantSource::Winner(away_winner),
    }
}

fn group_letters(groups: &[Group]) -> String {
    groups.iter().map(|group| group.letter()).collect()
}

trait GroupLetter {
    fn letter(self) -> char;
}

impl GroupLetter for Group {
    fn letter(self) -> char {
        match self {
            Group::A => 'A',
            Group::B => 'B',
            Group::C => 'C',
            Group::D => 'D',
            Group::E => 'E',
            Group::F => 'F',
            Group::G => 'G',
            Group::H => 'H',
            Group::I => 'I',
            Group::J => 'J',
            Group::K => 'K',
            Group::L => 'L',
        }
    }
}

#[derive(Clone, Default)]
struct StyledRow {
    spans: Vec<Span<'static>>,
    width: usize,
}

impl From<StyledRow> for Line<'static> {
    fn from(row: StyledRow) -> Self {
        Line::from(row.spans)
    }
}

#[derive(Default)]
struct StyledLine {
    spans: Vec<Span<'static>>,
    width: usize,
}

impl StyledLine {
    fn styled(text: impl Into<String>, style: Style) -> Self {
        let mut line = Self::default();
        line.push_styled(text, style);
        line
    }

    fn push_raw(&mut self, text: impl Into<String>) {
        self.push_styled(text, Style::default());
    }

    fn push_styled(&mut self, text: impl Into<String>, style: Style) {
        let text = text.into();
        self.width += text.chars().count();
        self.spans.push(Span::styled(text, style));
    }
}

fn put_line(rows: &mut [StyledRow], y: usize, x: usize, line: StyledLine) {
    let Some(row) = rows.get_mut(y) else {
        return;
    };

    if row.width < x {
        let gap = x - row.width;
        row.spans.push(Span::raw(" ".repeat(gap)));
        row.width = x;
    }

    row.width += line.width;
    row.spans.extend(line.spans);
}

fn center(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len >= width {
        return truncate(value, width);
    }

    let left = (width - len) / 2;
    let right = width - len - left;
    format!("{}{}{}", " ".repeat(left), value, " ".repeat(right))
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
    use crate::{
        app::App,
        config::WORLD_CUP_2026,
        data::repository::AppSnapshot,
        domain::{Confederation, Match, MatchId, MatchStatus, Stage, StandingRow, Team, TeamId},
    };

    #[test]
    fn bracket_uses_standings_and_best_third_placeholders() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![
                sample_team("mex", "Mexico", "MEX"),
                sample_team("can", "Canada", "CAN"),
            ],
            standings: vec![
                sample_standing(Group::A, "mex", "Mexico", 2),
                sample_standing(Group::B, "can", "Canada", 2),
            ],
            ..AppSnapshot::default()
        });

        let text = bracket_lines(&app, &[KnockoutRoundFilter::RoundOf32])
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(text.contains("M73"));
        assert!(text.contains("MEX"));
        assert!(text.contains("CAN"));
        assert!(text.contains("3ABCDF"));
    }

    #[test]
    fn group_position_participants_show_slot_and_dim_unconfirmed_teams() {
        let mut confirmed_mexico = sample_standing(Group::E, "mex", "Mexico", 1);
        confirmed_mexico.qualification_status = Some("ConfirmedQualified".to_string());
        let projected_canada = sample_standing(Group::A, "can", "Canada", 1);
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![
                sample_team("mex", "Mexico", "MEX"),
                sample_team("can", "Canada", "CAN"),
            ],
            standings: vec![confirmed_mexico, projected_canada],
            ..AppSnapshot::default()
        });

        let lines = bracket_lines(&app, &[KnockoutRoundFilter::RoundOf32]);
        let text = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("1E-MEX"));
        assert!(text.contains("1A-CAN"));

        let mexico_span = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "1E-MEX")
            .expect("confirmed group winner");
        assert_eq!(mexico_span.style.fg, Some(Color::White));

        let canada_span = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "1A-CAN")
            .expect("projected group winner");
        assert_eq!(canada_span.style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn group_position_participants_show_disqualified_teams_in_red() {
        let mut disqualified_usa = sample_standing(Group::E, "usa", "USA", 1);
        disqualified_usa.qualification_status = Some("ConfirmedDisqualified".to_string());
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![sample_team("usa", "USA", "USA")],
            standings: vec![disqualified_usa],
            ..AppSnapshot::default()
        });

        let usa_span = bracket_lines(&app, &[KnockoutRoundFilter::RoundOf32])
            .into_iter()
            .flat_map(|line| line.spans)
            .find(|span| span.content.as_ref() == "1E-USA")
            .expect("disqualified group seed");

        assert_eq!(usa_span.style.fg, Some(Color::Red));
    }

    #[test]
    fn bracket_renders_mirrored_left_and_right_rounds() {
        let app = App::new(WORLD_CUP_2026);
        let text = bracket_lines(&app, &KnockoutRoundFilter::SELECTABLE)
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert_eq!(text.matches("Round of 32").count(), 2);
        assert!(text.contains("M74"));
        assert!(text.contains("M76"));
        assert!(text.find("Final / Third") < text.rfind("Round of 32"));
    }

    #[test]
    fn single_round_selection_keeps_left_and_right_sides() {
        let app = App::new(WORLD_CUP_2026);
        let text = bracket_lines(&app, &[KnockoutRoundFilter::RoundOf32])
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert_eq!(text.matches("Round of 32").count(), 2);
        assert!(text.contains("M74"));
        assert!(text.contains("M76"));
    }

    #[test]
    fn bracket_right_aligns_short_upstream_placeholders() {
        let app = App::new(WORLD_CUP_2026);
        let lines = bracket_lines(&app, &[KnockoutRoundFilter::Final]);
        let text = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(text.contains("W101 |"));
        assert!(text.contains("W102 |"));
        assert!(!text.contains("W101  |"));
        assert!(!text.contains("W102  |"));

        let placeholder_span = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "W101")
            .expect("upstream placeholder");
        assert_eq!(placeholder_span.style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn final_and_third_place_use_distinct_border_colors_and_score_placeholders() {
        let app = App::new(WORLD_CUP_2026);
        let final_lines = bracket_lines(&app, &[KnockoutRoundFilter::Final]);
        let final_border = final_lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "|")
            .expect("final border");
        assert_eq!(final_border.style.fg, Some(Color::Yellow));

        let third_lines = bracket_lines(&app, &[KnockoutRoundFilter::ThirdPlace]);
        let third_border = third_lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "|")
            .expect("third-place border");
        assert_eq!(third_border.style.fg, Some(Color::Indexed(208)));

        let placeholder_score = final_lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "-")
            .expect("score placeholder");
        assert_eq!(placeholder_score.style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn bracket_renders_scores_and_favorite_underline_without_yellow_text() {
        let mut france = sample_team("fra", "France", "FRA");
        france.favorite = true;
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![sample_team("arg", "Argentina", "ARG"), france],
            matches: vec![sample_knockout_match()],
            ..AppSnapshot::default()
        });

        let lines = bracket_lines(&app, &[KnockoutRoundFilter::RoundOf32]);
        let text = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(text.contains("ARG"));
        assert!(text.contains("FRA"));
        assert!(text.contains("3"));
        assert!(text.contains("1"));

        let arg_span = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "ARG")
            .expect("winner team code");
        assert_eq!(arg_span.style.fg, Some(Color::Green));

        let arg_score_span = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "3")
            .expect("winner score");
        assert_eq!(arg_score_span.style.fg, Some(Color::Green));

        let france_span = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "FRA")
            .expect("favorite team code");
        assert_ne!(france_span.style.fg, Some(Color::Yellow));
        assert!(
            france_span
                .style
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    fn sample_team(id: &'static str, name: &'static str, abbreviation: &'static str) -> Team {
        Team {
            id: TeamId::from(id),
            name: name.to_string(),
            abbreviation: abbreviation.to_string(),
            country_code: abbreviation.to_string(),
            confederation: Confederation::Concacaf,
            flag_url_template: None,
            fifa_rank: None,
            fifa_ranking_points: None,
            favorite: false,
        }
    }

    fn sample_knockout_match() -> Match {
        Match {
            id: MatchId::from("m81"),
            match_number: 81,
            stage_id: Stage::RoundOf32.id(),
            stage_name: Stage::RoundOf32.name().to_string(),
            group_id: None,
            group_name: None,
            utc_start: "2026-07-01T12:00:00Z".parse().expect("timestamp"),
            local_start: None,
            home_team_id: Some(TeamId::from("arg")),
            away_team_id: Some(TeamId::from("fra")),
            home_team_name: "Argentina".to_string(),
            away_team_name: "France".to_string(),
            home_score: Some(3),
            away_score: Some(1),
            home_penalty_score: None,
            away_penalty_score: None,
            status: MatchStatus::FullTime,
            minute: None,
            stadium_name: None,
            attendance: None,
            winner_team_id: Some(TeamId::from("arg")),
        }
    }

    fn sample_standing(
        group: Group,
        team_id: &'static str,
        team_name: &'static str,
        position: u8,
    ) -> StandingRow {
        StandingRow {
            group_id: group.id(),
            group_name: group.name().to_string(),
            team_id: TeamId::from(team_id),
            team_name: team_name.to_string(),
            position,
            played: 3,
            won: 1,
            drawn: 1,
            lost: 1,
            goals_for: 3,
            goals_against: 3,
            goal_difference: 0,
            points: 4,
            qualification_status: None,
            fair_play: None,
        }
    }
}
