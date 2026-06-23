use crate::{
    config::TournamentConfig,
    data::{
        repository::AppSnapshot,
        sqlite::DatabaseInfo,
        sync::{RefreshEvent, ResourceKey},
    },
    domain::{
        Confederation, Group, GroupId, Match, MatchStatus, QualificationState, StandingRow, Team,
        TeamId, TimelineEvent,
    },
};
use jiff::{Timestamp, tz::TimeZone};

const STANDINGS_CONTENT_PREAMBLE_LINES: u16 = 4;
const MATCHES_CONTENT_PREAMBLE_LINES: u16 = 5;
const KNOCKOUT_CONTENT_PREAMBLE_LINES: u16 = 0;
const KNOCKOUT_COLUMN_WIDTH: u16 = 27;
const DEFAULT_HORIZONTAL_SCROLL_STEP: u16 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Home,
    Countries,
    Matches,
    Standings,
    Knockouts,
    Stats,
    Facts,
}

impl Screen {
    pub const COUNT: usize = 7;
    pub const ALL: [Self; 7] = [
        Self::Home,
        Self::Countries,
        Self::Matches,
        Self::Standings,
        Self::Knockouts,
        Self::Stats,
        Self::Facts,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Countries => "Countries",
            Self::Matches => "Matches",
            Self::Standings => "Standings",
            Self::Knockouts => "Knock-outs",
            Self::Stats => "Stats",
            Self::Facts => "Fun Facts",
        }
    }

    pub fn key_hint(self) -> &'static str {
        match self {
            Self::Home => "0",
            Self::Countries => "1",
            Self::Matches => "2",
            Self::Standings => "3",
            Self::Knockouts => "4",
            Self::Stats => "5",
            Self::Facts => "6",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|screen| *screen == self)
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeMode {
    Absolute,
    Relative,
}

impl TimeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Absolute => "absolute",
            Self::Relative => "relative",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Absolute => Self::Relative,
            Self::Relative => Self::Absolute,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortOrder {
    Asc,
    Desc,
}

impl SortOrder {
    pub fn label(self) -> &'static str {
        match self {
            Self::Asc => "A-Z",
            Self::Desc => "Z-A",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    QuitConfirm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyScope {
    Global,
    Screen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusPane {
    None,
    Content,
    Detail,
}

impl FocusPane {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Content => "content",
            Self::Detail => "detail",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScreenScroll {
    content: u16,
    detail: u16,
    content_x: u16,
    detail_x: u16,
}

impl KeyScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Screen => "screen",
        }
    }

    fn toggle(self) -> Self {
        match self {
            Self::Global => Self::Screen,
            Self::Screen => Self::Global,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceState {
    Empty,
    Cached,
    Refreshing,
    Offline,
    Error,
}

impl SourceState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Cached => "synced",
            Self::Refreshing => "refreshing",
            Self::Offline => "offline",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceStatus {
    pub state: SourceState,
    pub detail: String,
    pub last_updated: Option<String>,
    pub pending_refreshes: usize,
}

impl SourceStatus {
    fn initial() -> Self {
        Self {
            state: SourceState::Empty,
            detail: "data layer not wired yet".to_string(),
            last_updated: None,
            pending_refreshes: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CountriesFilter {
    All,
    Afc,
    Caf,
    Concacaf,
    Conmebol,
    Ofc,
    Uefa,
}

impl CountriesFilter {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Afc,
        Self::Caf,
        Self::Concacaf,
        Self::Conmebol,
        Self::Ofc,
        Self::Uefa,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Afc => "AFC",
            Self::Caf => "CAF",
            Self::Concacaf => "CONCACAF",
            Self::Conmebol => "CONMEBOL",
            Self::Ofc => "OFC",
            Self::Uefa => "UEFA",
        }
    }

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::All => "A[l]l",
            Self::Afc => "[A]FC",
            Self::Caf => "[C]AF",
            Self::Concacaf => "CO[N]CACAF",
            Self::Conmebol => "CON[M]EBOL",
            Self::Ofc => "O[F]C",
            Self::Uefa => "[U]EFA",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::All => 'l',
            Self::Afc => 'a',
            Self::Caf => 'c',
            Self::Concacaf => 'n',
            Self::Conmebol => 'm',
            Self::Ofc => 'f',
            Self::Uefa => 'u',
        }
    }

    pub fn is_all(self) -> bool {
        self == Self::All
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|filter| *filter == self)
            .unwrap_or(0)
    }

    pub fn confederation(self) -> Option<Confederation> {
        match self {
            Self::All => None,
            Self::Afc => Some(Confederation::Afc),
            Self::Caf => Some(Confederation::Caf),
            Self::Concacaf => Some(Confederation::Concacaf),
            Self::Conmebol => Some(Confederation::Conmebol),
            Self::Ofc => Some(Confederation::Ofc),
            Self::Uefa => Some(Confederation::Uefa),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatchesFilter {
    All,
    Today,
    Past,
    Future,
}

impl MatchesFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Today, Self::Past, Self::Future];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all dates",
            Self::Today => "today",
            Self::Past => "past",
            Self::Future => "future",
        }
    }

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::All => "all date[s]",
            Self::Today => "[t]oday",
            Self::Past => "[p]ast",
            Self::Future => "f[u]ture",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::All => 's',
            Self::Today => 't',
            Self::Past => 'p',
            Self::Future => 'u',
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimelineFilter {
    All,
    Goals,
    RedCards,
    YellowCards,
    Substitutions,
}

impl TimelineFilter {
    pub const SELECTABLE: [Self; 4] = [
        Self::Goals,
        Self::RedCards,
        Self::YellowCards,
        Self::Substitutions,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "all events",
            Self::Goals => "goals",
            Self::RedCards => "red cards",
            Self::YellowCards => "yellow cards",
            Self::Substitutions => "subs",
        }
    }

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Goals => "[g]oals",
            Self::RedCards => "[r]ed cards",
            Self::YellowCards => "[y]ellow cards",
            Self::Substitutions => "[s]ubs",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::All => 'a',
            Self::Goals => 'g',
            Self::RedCards => 'r',
            Self::YellowCards => 'y',
            Self::Substitutions => 's',
        }
    }

    fn index(self) -> usize {
        Self::SELECTABLE
            .iter()
            .position(|filter| *filter == self)
            .unwrap_or(usize::MAX)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandingsFilter {
    AllGroups,
    GroupA,
    GroupB,
    GroupC,
    GroupD,
    GroupE,
    GroupF,
    GroupG,
    GroupH,
    GroupI,
    GroupJ,
    GroupK,
    GroupL,
}

impl StandingsFilter {
    pub const ALL: [Self; 13] = [
        Self::AllGroups,
        Self::GroupA,
        Self::GroupB,
        Self::GroupC,
        Self::GroupD,
        Self::GroupE,
        Self::GroupF,
        Self::GroupG,
        Self::GroupH,
        Self::GroupI,
        Self::GroupJ,
        Self::GroupK,
        Self::GroupL,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AllGroups => "All groups",
            Self::GroupA => "Group A",
            Self::GroupB => "Group B",
            Self::GroupC => "Group C",
            Self::GroupD => "Group D",
            Self::GroupE => "Group E",
            Self::GroupF => "Group F",
            Self::GroupG => "Group G",
            Self::GroupH => "Group H",
            Self::GroupI => "Group I",
            Self::GroupJ => "Group J",
            Self::GroupK => "Group K",
            Self::GroupL => "Group L",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::AllGroups => 'r',
            Self::GroupA => 'a',
            Self::GroupB => 'b',
            Self::GroupC => 'c',
            Self::GroupD => 'd',
            Self::GroupE => 'e',
            Self::GroupF => 'f',
            Self::GroupG => 'g',
            Self::GroupH => 'h',
            Self::GroupI => 'i',
            Self::GroupJ => 'j',
            Self::GroupK => 'k',
            Self::GroupL => 'l',
        }
    }

    pub fn is_all(self) -> bool {
        self == Self::AllGroups
    }

    pub fn group_id(self) -> Option<GroupId> {
        let group = match self {
            Self::AllGroups => return None,
            Self::GroupA => Group::A,
            Self::GroupB => Group::B,
            Self::GroupC => Group::C,
            Self::GroupD => Group::D,
            Self::GroupE => Group::E,
            Self::GroupF => Group::F,
            Self::GroupG => Group::G,
            Self::GroupH => Group::H,
            Self::GroupI => Group::I,
            Self::GroupJ => Group::J,
            Self::GroupK => Group::K,
            Self::GroupL => Group::L,
        };

        Some(group.id())
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|filter| *filter == self)
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnockoutRoundFilter {
    All,
    RoundOf32,
    RoundOf16,
    QuarterFinal,
    SemiFinal,
    ThirdPlace,
    Final,
}

impl KnockoutRoundFilter {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::RoundOf32,
        Self::RoundOf16,
        Self::QuarterFinal,
        Self::SemiFinal,
        Self::ThirdPlace,
        Self::Final,
    ];

    pub const SELECTABLE: [Self; 6] = [
        Self::RoundOf32,
        Self::RoundOf16,
        Self::QuarterFinal,
        Self::SemiFinal,
        Self::ThirdPlace,
        Self::Final,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All rounds",
            Self::RoundOf32 => "Round of 32",
            Self::RoundOf16 => "Round of 16",
            Self::QuarterFinal => "Quarter-finals",
            Self::SemiFinal => "Semi-finals",
            Self::ThirdPlace => "Third place",
            Self::Final => "Final",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::All => 'a',
            Self::RoundOf32 => 'r',
            Self::RoundOf16 => 'o',
            Self::QuarterFinal => 'u',
            Self::SemiFinal => 's',
            Self::ThirdPlace => 't',
            Self::Final => 'f',
        }
    }

    pub fn is_all(self) -> bool {
        self == Self::All
    }

    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|filter| *filter == self)
            .unwrap_or(0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatsTab {
    All,
    Records,
    Goals,
    Fouls,
    Passes,
}

impl StatsTab {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Records,
        Self::Goals,
        Self::Fouls,
        Self::Passes,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Records => "Records",
            Self::Goals => "Goals",
            Self::Fouls => "Fouls",
            Self::Passes => "Passes",
        }
    }

    pub fn menu_label(self) -> &'static str {
        match self {
            Self::All => "[A]ll",
            Self::Records => "[R]ecords",
            Self::Goals => "[G]oals",
            Self::Fouls => "[F]ouls",
            Self::Passes => "[P]asses",
        }
    }

    pub fn shortcut(self) -> char {
        match self {
            Self::All => 'a',
            Self::Records => 'r',
            Self::Goals => 'g',
            Self::Fouls => 'f',
            Self::Passes => 'p',
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppCommand {
    Navigate(Screen),
    Refresh,
    ToggleFavoriteOnly,
    ToggleTimeMode,
    ToggleKeyScope,
    ToggleHelp,
    CloseOverlay,
    Quit,
    ConfirmQuit,
    FocusNext,
    FocusPrevious,
    ScrollUp { max: u16, visible_lines: u16 },
    ScrollDown { max: u16, visible_lines: u16 },
    ScrollLeft { max: u16 },
    ScrollRight { max: u16 },
    OpenDetails,
    ToggleSelectedCountryFavorite,
    ToggleCountryFilter(CountriesFilter),
    SelectMatchesFilter(MatchesFilter),
    ToggleMatchGroup(StandingsFilter),
    ToggleStandingGroup(StandingsFilter),
    ToggleKnockoutRound(KnockoutRoundFilter),
    SelectStatsTab(StatsTab),
    ToggleTimelineFilter(TimelineFilter),
    ToggleSort,
    StartSearch,
    SearchInput(char),
    SearchBackspace,
    SubmitSearch,
    OpenSelectedStandingGroupMatches,
}

#[derive(Clone, Debug, PartialEq)]
pub struct App {
    config: TournamentConfig,
    running: bool,
    screen: Screen,
    country_filters: Vec<CountriesFilter>,
    matches_filter: MatchesFilter,
    match_groups: Vec<StandingsFilter>,
    standing_groups: Vec<StandingsFilter>,
    knockout_rounds: Vec<KnockoutRoundFilter>,
    stats_tab: StatsTab,
    timeline_filters: Vec<TimelineFilter>,
    countries_sort: SortOrder,
    standings_sort: SortOrder,
    favorite_only: bool,
    time_mode: TimeMode,
    input_mode: InputMode,
    key_scope: KeyScope,
    focus_pane: FocusPane,
    scroll: [ScreenScroll; Screen::COUNT],
    help_open: bool,
    detail_open: bool,
    search_query: String,
    country_cursor: usize,
    match_cursor: usize,
    standing_group_cursor: usize,
    knockout_column_cursor: usize,
    knockout_match_cursor: usize,
    source_status: SourceStatus,
    database_info: Option<DatabaseInfo>,
    snapshot: AppSnapshot,
    message: String,
}

impl App {
    pub fn new(config: TournamentConfig) -> Self {
        Self {
            config,
            running: true,
            screen: Screen::Home,
            country_filters: Vec::new(),
            matches_filter: MatchesFilter::Future,
            match_groups: Vec::new(),
            standing_groups: Vec::new(),
            knockout_rounds: Vec::new(),
            stats_tab: StatsTab::All,
            timeline_filters: Vec::new(),
            countries_sort: SortOrder::Asc,
            standings_sort: SortOrder::Asc,
            favorite_only: false,
            time_mode: TimeMode::Absolute,
            input_mode: InputMode::Normal,
            key_scope: KeyScope::Global,
            focus_pane: FocusPane::None,
            scroll: [ScreenScroll::default(); Screen::COUNT],
            help_open: false,
            detail_open: false,
            search_query: String::new(),
            country_cursor: 0,
            match_cursor: 0,
            standing_group_cursor: 0,
            knockout_column_cursor: 0,
            knockout_match_cursor: 0,
            source_status: SourceStatus::initial(),
            database_info: None,
            snapshot: AppSnapshot::default(),
            message: "Ready. Data persistence and refresh arrive in the next milestones."
                .to_string(),
        }
    }

    pub fn config(&self) -> TournamentConfig {
        self.config
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn normalized_search_query(&self) -> String {
    self.search_query.trim().to_lowercase()
    }

    #[cfg(test)]
    pub fn country_filters(&self) -> &[CountriesFilter] {
        &self.country_filters
    }

    pub fn country_filter_label(&self) -> String {
        if self.country_filters.is_empty() {
            return "All".to_string();
        }

        self.country_filters
            .iter()
            .map(|filter| filter.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn matches_filter(&self) -> MatchesFilter {
        self.matches_filter
    }

    pub fn match_group_label(&self) -> String {
        if self.match_groups.is_empty() {
            return "All groups".to_string();
        }

        self.match_groups
            .iter()
            .map(|group| group.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[cfg(test)]
    pub fn standing_groups(&self) -> &[StandingsFilter] {
        &self.standing_groups
    }

    pub fn standing_group_label(&self) -> String {
        if self.standing_groups.is_empty() {
            return "All groups".to_string();
        }

        self.standing_groups
            .iter()
            .map(|group| group.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[cfg(test)]
    pub fn knockout_rounds(&self) -> &[KnockoutRoundFilter] {
        &self.knockout_rounds
    }

    pub fn knockout_round_label(&self) -> String {
        if self.knockout_rounds.is_empty() {
            return "All rounds".to_string();
        }

        self.knockout_rounds
            .iter()
            .map(|round| round.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn visible_knockout_rounds(&self) -> Vec<KnockoutRoundFilter> {
        if self.knockout_rounds.is_empty() {
            return KnockoutRoundFilter::SELECTABLE.to_vec();
        }

        self.knockout_rounds.clone()
    }

    pub fn knockout_column_cursor(&self) -> usize {
        self.knockout_column_cursor
            .min(self.knockout_column_count().saturating_sub(1))
    }

    pub fn knockout_match_cursor(&self) -> usize {
        self.knockout_match_cursor.min(
            self.knockout_selected_column_match_count()
                .saturating_sub(1),
        )
    }

    pub fn stats_tab(&self) -> StatsTab {
        self.stats_tab
    }

    pub fn timeline_filters(&self) -> &[TimelineFilter] {
        &self.timeline_filters
    }

    pub fn timeline_filter_label(&self) -> String {
        if self.timeline_filters.is_empty() {
            return TimelineFilter::All.label().to_string();
        }

        self.timeline_filters
            .iter()
            .map(|filter| filter.label())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn countries_sort(&self) -> SortOrder {
        self.countries_sort
    }

    pub fn standings_sort(&self) -> SortOrder {
        self.standings_sort
    }

    pub fn favorite_only(&self) -> bool {
        self.favorite_only
    }

    pub fn time_mode(&self) -> TimeMode {
        self.time_mode
    }

    pub fn input_mode(&self) -> InputMode {
        self.input_mode
    }

    pub fn key_scope(&self) -> KeyScope {
        self.key_scope
    }

    pub fn focus_pane(&self) -> FocusPane {
        self.focus_pane
    }

    pub fn content_scroll(&self) -> u16 {
        self.current_scroll().content
    }

    pub fn detail_scroll(&self) -> u16 {
        self.current_scroll().detail
    }

    pub fn content_horizontal_scroll(&self) -> u16 {
        self.current_scroll().content_x
    }

    pub fn help_open(&self) -> bool {
        self.help_open
    }

    pub fn detail_open(&self) -> bool {
        self.detail_open
    }

    pub fn source_status(&self) -> &SourceStatus {
        &self.source_status
    }

    pub fn database_info(&self) -> Option<&DatabaseInfo> {
        self.database_info.as_ref()
    }

    pub fn team_count(&self) -> usize {
        self.snapshot.teams.len()
    }

    pub fn match_count(&self) -> usize {
        self.snapshot.matches.len()
    }

    pub fn knockout_match_count(&self) -> usize {
        self.snapshot
            .matches
            .iter()
            .filter(|match_| match_.match_number >= 73 && match_.match_number <= 104)
            .count()
    }

    pub fn standing_count(&self) -> usize {
        self.snapshot.standings.len()
    }

    pub fn visible_teams(&self) -> Vec<&Team> {
        let query = self.normalized_search_query();
        let mut teams = self
            .snapshot
            .teams
            .iter()
            .filter(|team| self.team_matches_country_filters(team))
            .filter(|team| !self.favorite_only || team.favorite)
            .filter(|team| {
                query.is_empty()
                    || contains_query(&team.name, &query)
                    || contains_query(&team.abbreviation, &query)
                    || contains_query(&team.country_code, &query)
                    || contains_query(team.confederation.code(), &query)
            })
            .collect::<Vec<_>>();

        teams.sort_by(|left, right| {
            let ordering = left.name.to_lowercase().cmp(&right.name.to_lowercase());
            match self.countries_sort {
                SortOrder::Asc => ordering,
                SortOrder::Desc => ordering.reverse(),
            }
        });

        teams
    }

    pub fn visible_matches(&self) -> Vec<&Match> {
        let query = self.normalized_search_query();
        let mut matches = self
            .snapshot
            .matches
            .iter()
            .filter(|match_| self.match_matches_date_filter(match_))
            .filter(|match_| self.match_matches_group_filters(match_))
            .filter(|match_| !self.favorite_only || self.match_has_favorite_team(match_))
            .filter(|match_| {
                query.is_empty()
                    || contains_query(&match_.home_team_name, &query)
                    || contains_query(&match_.away_team_name, &query)
                    || match_
                        .group_name
                        .as_ref()
                        .is_some_and(|value| contains_query(value, &query))
                    || contains_query(&match_.stage_name, &query)
                    || match_
                        .stadium_name
                        .as_ref()
                        .is_some_and(|value| contains_query(value, &query))
            })
            .collect::<Vec<_>>();

        matches.sort_by_key(|match_| (match_.utc_start, match_.match_number));
        if self.matches_filter() == MatchesFilter::Past {
            matches.reverse();
        }
        matches
    }

    pub fn visible_standings(&self) -> Vec<&StandingRow> {
      let query = self.normalized_search_query();
        let mut standings = self
            .snapshot
            .standings
            .iter()
            .filter(|row| self.standing_matches_group_filters(row))
            .filter(|row| !self.favorite_only || self.standing_has_favorite_team(row))
            .filter(|row| {
                query.is_empty()
                    || contains_query(&row.team_name, &query)
                    || contains_query(&row.group_name, &query)
            })
            .collect::<Vec<_>>();

        standings.sort_by(|left, right| {
            let group_order = match self.standings_sort {
                SortOrder::Asc => standing_group_sort_key(&left.group_id)
                    .cmp(&standing_group_sort_key(&right.group_id)),
                SortOrder::Desc => standing_group_sort_key(&right.group_id)
                    .cmp(&standing_group_sort_key(&left.group_id)),
            };

            group_order
                .then(left.position.cmp(&right.position))
                .then_with(|| {
                    left.team_name
                        .to_lowercase()
                        .cmp(&right.team_name.to_lowercase())
                })
        });

        standings
    }

    pub fn selected_standing_group_id(&self) -> Option<GroupId> {
        let groups = self.visible_standing_group_ids();
        groups
            .get(
                self.standing_group_cursor
                    .min(groups.len().saturating_sub(1)),
            )
            .cloned()
    }

    pub fn selected_match(&self) -> Option<&Match> {
        let matches = self.visible_matches();
        if matches.is_empty() {
            return None;
        }

        matches
            .get(self.match_cursor.min(matches.len() - 1))
            .copied()
    }

    pub fn selected_match_id(&self) -> Option<crate::domain::MatchId> {
        self.selected_match().map(|match_| match_.id.clone())
    }

    pub fn selected_match_timeline_events(&self) -> Vec<&TimelineEvent> {
        let Some(match_id) = self.selected_match_id() else {
            return Vec::new();
        };

        let mut events = self
            .snapshot
            .timeline_events
            .iter()
            .filter(|event| event.match_id == match_id)
            .collect::<Vec<_>>();
        events.sort_by_key(|event| std::cmp::Reverse(event.event_index));
        events
    }

    pub fn selected_match_has_timeline(&self) -> bool {
        let Some(match_id) = self.selected_match_id() else {
            return false;
        };

        self.snapshot
            .timeline_events
            .iter()
            .any(|event| event.match_id == match_id)
    }

    pub fn selected_match_is_future(&self) -> bool {
        self.selected_match()
            .is_some_and(|match_| match_is_future(match_))
    }

    pub fn selected_country_id(&self) -> Option<TeamId> {
        self.selected_country().map(|team| team.id.clone())
    }

    pub fn selected_country_action(&self) -> Option<(TeamId, String, bool)> {
        self.selected_country()
            .map(|team| (team.id.clone(), team.name.clone(), team.favorite))
    }

    pub fn selected_country(&self) -> Option<&Team> {
        let teams = self.visible_teams();
        if teams.is_empty() {
            return None;
        }

        teams.get(self.country_cursor.min(teams.len() - 1)).copied()
    }

    pub fn match_time_label(&self, match_: &Match) -> String {
        match self.time_mode {
            TimeMode::Absolute => match_
                .utc_start
                .to_zoned(TimeZone::system())
                .strftime("%Y-%m-%d %H:%M")
                .to_string(),
            TimeMode::Relative => relative_time_label(match_.utc_start, Timestamp::now()),
        }
    }

    pub fn match_date_key(&self, match_: &Match) -> String {
        match_
            .utc_start
            .to_zoned(TimeZone::system())
            .strftime("%Y-%m-%d")
            .to_string()
    }

    pub fn last_sync_label(&self) -> String {
        let Some(value) = self.source_status.last_updated.as_deref() else {
            return "never".to_string();
        };

        let Ok(timestamp) = value.parse::<Timestamp>() else {
            return value.to_string();
        };

        match self.time_mode {
            TimeMode::Absolute => timestamp
                .to_zoned(TimeZone::system())
                .strftime("%d/%m %H:%M")
                .to_string(),
            TimeMode::Relative => relative_time_label(timestamp, Timestamp::now()),
        }
    }

    pub fn match_team_code(&self, team_id: Option<&TeamId>, fallback: &str) -> String {
        team_id
            .and_then(|team_id| self.snapshot.teams.iter().find(|team| &team.id == team_id))
            .map(|team| team.abbreviation.clone())
            .unwrap_or_else(|| fallback_code(fallback))
    }

    pub fn match_by_number(&self, match_number: u16) -> Option<&Match> {
        self.snapshot
            .matches
            .iter()
            .find(|match_| match_.match_number == match_number)
    }

    pub fn standing_for_group_position(&self, group: Group, position: u8) -> Option<&StandingRow> {
        self.snapshot
            .standings
            .iter()
            .find(|row| row.group_id == group.id() && row.position == position)
    }

    pub fn team_is_favorite(&self, team_id: &TeamId) -> bool {
        self.snapshot
            .teams
            .iter()
            .any(|team| team.favorite && &team.id == team_id)
    }

    pub fn match_status_label(&self, status: MatchStatus) -> &'static str {
        match status {
            MatchStatus::Scheduled => "scheduled",
            MatchStatus::Live => "live",
            MatchStatus::FullTime => "full-time",
            MatchStatus::ExtraTime => "extra-time",
            MatchStatus::Penalties => "penalties",
            MatchStatus::Postponed => "postponed",
            MatchStatus::Cancelled => "cancelled",
            MatchStatus::Unknown(_) => "unknown",
        }
    }

    pub fn match_includes_favorite_team(&self, match_: &Match) -> bool {
        self.match_has_favorite_team(match_)
    }

    pub fn standing_team_is_favorite(&self, team_id: &TeamId) -> bool {
        self.snapshot
            .teams
            .iter()
            .any(|team| team.favorite && &team.id == team_id)
    }

    pub fn standing_row_is_advancing(&self, row: &StandingRow) -> bool {
        match row.qualification_state() {
            QualificationState::Disqualified => return false,
            QualificationState::Qualified => return true,
            QualificationState::Open => {}
        }

        if row.position <= 2 {
            return true;
        }
        if row.position != 3 {
            return false;
        }

        self.best_third_team_ids()
            .iter()
            .any(|team_id| team_id == &row.team_id)
    }

    pub fn standing_row_qualification_state(&self, row: &StandingRow) -> QualificationState {
        row.qualification_state()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn set_database_info(&mut self, info: DatabaseInfo) {
        self.source_status.detail = format!("db:{}", info.location.label());
        self.message = format!("Database ready: {}.", info.path.display());
        self.database_info = Some(info);
    }

    pub fn set_last_sync(&mut self, last_sync: Option<String>) {
        self.source_status.last_updated = last_sync;
    }

    pub fn set_snapshot(&mut self, snapshot: AppSnapshot) {
        let team_count = snapshot.teams.len();
        let match_count = snapshot.matches.len();
        let standing_count = snapshot.standings.len();
        self.snapshot = snapshot;
        self.clamp_country_cursor();
        self.clamp_match_cursor();
        self.clamp_standing_group_cursor();
        if team_count > 0 || match_count > 0 || standing_count > 0 {
            self.source_status.state = SourceState::Cached;
            self.message = format!("Loaded data");
        }
    }

    pub fn set_snapshot_error(&mut self, error: String) {
        self.source_status.state = SourceState::Error;
        self.message = format!("Loading saved data failed: {error}");
    }

    pub fn refresh_resources(&self) -> Vec<ResourceKey> {
        match self.screen {
            Screen::Home => {
                let mut resources = vec![
                    ResourceKey::Teams,
                    ResourceKey::Stages,
                    ResourceKey::Matches,
                    ResourceKey::TopScorers,
                ];
                resources.extend(ResourceKey::all_standings_groups());
                resources
            }
            Screen::Countries => vec![ResourceKey::Teams, ResourceKey::Stages],
            Screen::Matches | Screen::Knockouts => matches_with_all_standings_resources(),
            Screen::Standings => {
                if self.standing_groups.is_empty() {
                    ResourceKey::all_standings_groups()
                } else {
                    self.standing_groups
                        .iter()
                        .filter_map(|group| group.group_id())
                        .map(ResourceKey::StandingsGroup)
                        .collect()
                }
            }
            Screen::Stats => vec![ResourceKey::TopScorers],
            Screen::Facts => Vec::new(),
        }
    }

    pub fn startup_refresh_resources(&self) -> Vec<ResourceKey> {
        self.automatic_refresh_resources()
    }

    pub fn background_refresh_resources(&self) -> Vec<ResourceKey> {
        self.automatic_refresh_resources()
    }

    fn automatic_refresh_resources(&self) -> Vec<ResourceKey> {
        let mut resources = Vec::new();
        if self.team_count() == 0 {
            resources.push(ResourceKey::Teams);
        }
        if self.matches_need_automatic_refresh() {
            resources.push(ResourceKey::Matches);
            resources.extend(self.standing_groups_needing_match_refresh());
        }
        resources
    }

    fn matches_need_automatic_refresh(&self) -> bool {
        if self.match_count() == 0 {
            return true;
        }

        let now = Timestamp::now();
        self.snapshot
            .matches
            .iter()
            .any(|match_| match_needs_automatic_refresh(match_, now))
    }

    fn standing_groups_needing_match_refresh(&self) -> Vec<ResourceKey> {
        let now = Timestamp::now();
        let mut group_ids = Vec::<GroupId>::new();

        for match_ in &self.snapshot.matches {
            let Some(group_id) = match_.group_id.as_ref() else {
                continue;
            };
            if match_needs_automatic_refresh(match_, now) && !group_ids.contains(group_id) {
                group_ids.push(group_id.clone());
            }
        }

        group_ids
            .into_iter()
            .map(ResourceKey::StandingsGroup)
            .collect()
    }

    pub fn handle_refresh_event(&mut self, event: RefreshEvent) {
        match event {
            RefreshEvent::Started { resource } => {
                self.source_status.state = SourceState::Refreshing;
                self.source_status.pending_refreshes =
                    self.source_status.pending_refreshes.saturating_add(1);
                self.message = format!("Syncing {}", resource.label());
            }
            RefreshEvent::Deduped { resource } => {
                if self.source_status.pending_refreshes == 0 {
                    self.source_status.state = SourceState::Cached;
                }
                self.message = format!("{} sync already in flight.", resource.label());
            }
            RefreshEvent::Cooldown { resource } => {
                if self.source_status.pending_refreshes == 0 {
                    self.source_status.state = SourceState::Cached;
                }
                self.message = format!("{} sync is cooling down.", resource.label());
            }
            RefreshEvent::Succeeded { resource, at } => {
                self.source_status.pending_refreshes =
                    self.source_status.pending_refreshes.saturating_sub(1);
                self.source_status.last_updated = Some(at);
                if self.source_status.pending_refreshes == 0 {
                    self.source_status.state = SourceState::Cached;
                }
                self.message = format!("Synced {}.", resource.label());
            }
            RefreshEvent::Failed { resource, error } => {
                self.source_status.pending_refreshes =
                    self.source_status.pending_refreshes.saturating_sub(1);
                if self.source_status.pending_refreshes == 0 {
                    self.source_status.state = SourceState::Error;
                }
                self.message = format!("{} sync failed: {error}", resource.label());
            }
            RefreshEvent::Offline { resource, error } => {
                if self.source_status.pending_refreshes == 0 {
                    self.source_status.state = SourceState::Offline;
                }
                self.message = format!("{} sync skipped: offline ({error})", resource.label());
            }
        }
    }

    fn current_scroll(&self) -> ScreenScroll {
        self.scroll[self.screen.index()]
    }

    fn current_scroll_mut(&mut self) -> &mut ScreenScroll {
        &mut self.scroll[self.screen.index()]
    }

    fn team_matches_country_filters(&self, team: &Team) -> bool {
        if self.country_filters.is_empty() {
            return true;
        }

        self.country_filters
            .iter()
            .filter_map(|filter| filter.confederation())
            .any(|confederation| confederation == team.confederation)
    }

    fn match_matches_date_filter(&self, match_: &Match) -> bool {
        match self.matches_filter {
            MatchesFilter::All => true,
            MatchesFilter::Today => {
                let today = Timestamp::now().to_zoned(TimeZone::system()).date();
                match_.utc_start.to_zoned(TimeZone::system()).date() == today
            }
            MatchesFilter::Past => match_.utc_start < Timestamp::now(),
            MatchesFilter::Future => match_.utc_start > Timestamp::now(),
        }
    }

    fn match_matches_group_filters(&self, match_: &Match) -> bool {
        if self.match_groups.is_empty() {
            return true;
        }

        self.match_groups
            .iter()
            .filter_map(|group| group.group_id())
            .any(|group_id| match_.group_id.as_ref() == Some(&group_id))
    }

    fn match_has_favorite_team(&self, match_: &Match) -> bool {
        self.snapshot.teams.iter().any(|team| {
            team.favorite
                && (match_.home_team_id.as_ref() == Some(&team.id)
                    || match_.away_team_id.as_ref() == Some(&team.id))
        })
    }

    fn standing_matches_group_filters(&self, row: &StandingRow) -> bool {
        if self.standing_groups.is_empty() {
            return true;
        }

        self.standing_groups
            .iter()
            .filter_map(|group| group.group_id())
            .any(|group_id| row.group_id == group_id)
    }

    fn standing_has_favorite_team(&self, row: &StandingRow) -> bool {
        self.standing_team_is_favorite(&row.team_id)
    }

    fn visible_match_count(&self) -> usize {
        self.visible_matches().len()
    }

    fn visible_standing_group_ids(&self) -> Vec<GroupId> {
        let mut groups = Vec::<GroupId>::new();
        for row in self.visible_standings() {
            if groups.last() != Some(&row.group_id) {
                groups.push(row.group_id.clone());
            }
        }
        groups
    }

    fn visible_standing_group_count(&self) -> usize {
        self.visible_standing_group_ids().len()
    }

    fn visible_knockout_columns(&self) -> Vec<KnockoutColumn> {
        let rounds = self.visible_knockout_rounds();

        let mut columns = Vec::new();
        if rounds.contains(&KnockoutRoundFilter::RoundOf32) {
            columns.push(KnockoutColumn::new(
                "left Round of 32",
                KnockoutColumnKind::RoundOf32,
                8,
            ));
        }
        if rounds.contains(&KnockoutRoundFilter::RoundOf16) {
            columns.push(KnockoutColumn::new(
                "left Round of 16",
                KnockoutColumnKind::RoundOf16,
                4,
            ));
        }
        if rounds.contains(&KnockoutRoundFilter::QuarterFinal) {
            columns.push(KnockoutColumn::new(
                "left quarter-finals",
                KnockoutColumnKind::QuarterFinal,
                2,
            ));
        }
        if rounds.contains(&KnockoutRoundFilter::SemiFinal) {
            columns.push(KnockoutColumn::new(
                "left semi-final",
                KnockoutColumnKind::SemiFinal,
                1,
            ));
        }

        let center_count = usize::from(rounds.contains(&KnockoutRoundFilter::Final))
            + usize::from(rounds.contains(&KnockoutRoundFilter::ThirdPlace));
        if center_count > 0 {
            columns.push(KnockoutColumn::new(
                "final / third",
                KnockoutColumnKind::Center,
                center_count,
            ));
        }

        if rounds.contains(&KnockoutRoundFilter::SemiFinal) {
            columns.push(KnockoutColumn::new(
                "right semi-final",
                KnockoutColumnKind::SemiFinal,
                1,
            ));
        }
        if rounds.contains(&KnockoutRoundFilter::QuarterFinal) {
            columns.push(KnockoutColumn::new(
                "right quarter-finals",
                KnockoutColumnKind::QuarterFinal,
                2,
            ));
        }
        if rounds.contains(&KnockoutRoundFilter::RoundOf16) {
            columns.push(KnockoutColumn::new(
                "right Round of 16",
                KnockoutColumnKind::RoundOf16,
                4,
            ));
        }
        if rounds.contains(&KnockoutRoundFilter::RoundOf32) {
            columns.push(KnockoutColumn::new(
                "right Round of 32",
                KnockoutColumnKind::RoundOf32,
                8,
            ));
        }

        columns
    }

    fn knockout_column_count(&self) -> usize {
        self.visible_knockout_columns().len().max(1)
    }

    fn knockout_selected_column_match_count(&self) -> usize {
        self.visible_knockout_columns()
            .get(self.knockout_column_cursor)
            .map(|column| column.match_count)
            .unwrap_or(0)
    }

    fn best_third_team_ids(&self) -> Vec<TeamId> {
        let mut rows = self
            .snapshot
            .standings
            .iter()
            .filter(|row| row.position == 3)
            .collect::<Vec<_>>();

        rows.sort_by(|left, right| {
            right
                .points
                .cmp(&left.points)
                .then(right.goal_difference.cmp(&left.goal_difference))
                .then(right.goals_for.cmp(&left.goals_for))
                .then(standing_fair_play(right).cmp(&standing_fair_play(left)))
                .then(
                    team_fifa_rank(self, &left.team_id).cmp(&team_fifa_rank(self, &right.team_id)),
                )
                .then_with(|| {
                    left.team_name
                        .to_lowercase()
                        .cmp(&right.team_name.to_lowercase())
                })
        });

        rows.into_iter()
            .take(8)
            .map(|row| row.team_id.clone())
            .collect()
    }

    pub fn handle_command(&mut self, command: AppCommand) {
        match command {
            AppCommand::Navigate(screen) => self.navigate(screen),
            AppCommand::Refresh => self.refresh_current_screen(),
            AppCommand::ToggleFavoriteOnly => self.toggle_favorite_only(),
            AppCommand::ToggleTimeMode => self.toggle_time_mode(),
            AppCommand::ToggleKeyScope => self.toggle_key_scope(),
            AppCommand::ToggleHelp => self.toggle_help(),
            AppCommand::CloseOverlay => self.close_overlay(),
            AppCommand::Quit => self.open_quit_confirm(),
            AppCommand::ConfirmQuit => self.running = false,
            AppCommand::FocusNext => self.focus_next(),
            AppCommand::FocusPrevious => self.focus_previous(),
            AppCommand::ScrollUp { max, visible_lines } => self.scroll_up(max, visible_lines),
            AppCommand::ScrollDown { max, visible_lines } => self.scroll_down(max, visible_lines),
            AppCommand::ScrollLeft { max } => self.scroll_left(max),
            AppCommand::ScrollRight { max } => self.scroll_right(max),
            AppCommand::OpenDetails => self.open_details(),
            AppCommand::ToggleSelectedCountryFavorite => self.toggle_selected_country_favorite(),
            AppCommand::ToggleCountryFilter(filter) => self.toggle_country_filter(filter),
            AppCommand::SelectMatchesFilter(filter) => self.select_matches_filter(filter),
            AppCommand::ToggleMatchGroup(group) => self.toggle_match_group(group),
            AppCommand::ToggleStandingGroup(group) => self.toggle_standing_group(group),
            AppCommand::ToggleKnockoutRound(round) => self.toggle_knockout_round(round),
            AppCommand::SelectStatsTab(tab) => self.select_stats_tab(tab),
            AppCommand::ToggleTimelineFilter(filter) => self.toggle_timeline_filter(filter),
            AppCommand::ToggleSort => self.toggle_sort(),
            AppCommand::StartSearch => self.start_search(),
            AppCommand::SearchInput(character) => self.search_input(character),
            AppCommand::SearchBackspace => self.search_backspace(),
            AppCommand::SubmitSearch => self.submit_search(),
            AppCommand::OpenSelectedStandingGroupMatches => {
                self.open_selected_standing_group_matches()
            }
        }
    }

    fn navigate(&mut self, screen: Screen) {
        self.screen = screen;
        self.input_mode = InputMode::Normal;
        self.help_open = false;
        self.detail_open = false;
        self.focus_pane = FocusPane::None;
        self.message = screen.title().to_string();
    }

    fn refresh_current_screen(&mut self) {
        let resource_count = self.refresh_resources().len();
        if resource_count == 0 {
            self.message = format!("{} has no sync resources yet.", self.screen.title());
            return;
        }

        self.source_status.state = SourceState::Refreshing;
        self.message = format!(
            "{} sync requested for {resource_count} resource(s).",
            self.screen.title(),
        );
    }

    fn toggle_favorite_only(&mut self) {
        self.favorite_only = !self.favorite_only;
        if matches!(self.screen, Screen::Countries) {
            self.reset_country_selection();
        }
        if matches!(self.screen, Screen::Standings) {
            self.reset_standing_group_selection();
        }
        let state = if self.favorite_only {
            "enabled"
        } else {
            "disabled"
        };
        self.message = format!("Favorites filter {state}.");
    }

    fn toggle_time_mode(&mut self) {
        self.time_mode = self.time_mode.toggle();
        self.message = format!("Time display: {}.", self.time_mode.label());
    }

    fn toggle_key_scope(&mut self) {
        self.key_scope = self.key_scope.toggle();
    }

    fn toggle_help(&mut self) {
        if self.input_mode == InputMode::QuitConfirm {
            self.input_mode = InputMode::Normal;
        }
        self.help_open = !self.help_open;
        self.input_mode = InputMode::Normal;
        self.message = if self.help_open {
            "Help overlay open. Press Esc to close.".to_string()
        } else {
            "Help overlay closed.".to_string()
        };
    }

    fn close_overlay(&mut self) {
        if self.input_mode == InputMode::QuitConfirm {
            self.input_mode = InputMode::Normal;
            self.message = "Quit cancelled.".to_string();
            return;
        }

        if self.input_mode == InputMode::Search {
            self.input_mode = InputMode::Normal;
            self.message = "Search closed.".to_string();
            return;
        }

        if self.help_open {
            self.help_open = false;
            self.message = "Help overlay closed.".to_string();
            return;
        }

        if self.detail_open {
            self.detail_open = false;
            self.message = "Details closed.".to_string();
            if self.screen() == Screen::Matches {
                self.focus_previous();
            }
            return;
        }

        self.message = "Nothing to close.".to_string();
    }

    fn open_quit_confirm(&mut self) {
        self.input_mode = InputMode::QuitConfirm;
        self.help_open = false;
        self.message = "Confirm quit: q/y/Enter to quit, n/Esc to stay.".to_string();
    }

    fn open_details(&mut self) {
        if matches!(self.screen, Screen::Home) {
            self.message = "Home has no detail row.".to_string();
            return;
        }
        if matches!(self.screen, Screen::Standings) {
            self.detail_open = false;
            self.message =
                "Standings has no detail pane. Focus a group and press m for matches.".to_string();
            return;
        }

        if matches!(self.screen, Screen::Matches) {
            if self.selected_match_is_future() {
                self.detail_open = false;
                self.message = "Future matches have no details yet.".to_string();
                return;
            }
            self.reset_match_detail_view();
        }

        self.detail_open = true;
        self.focus_pane = FocusPane::Detail;
        self.message = match self.screen {
            Screen::Countries => self
                .selected_country()
                .map(|team| format!("Country details: {}.", team.name))
                .unwrap_or_else(|| "No country row selected.".to_string()),
            Screen::Matches => self
                .selected_match()
                .map(|match_| {
                    format!(
                        "Match details: {} vs {}.",
                        match_.home_team_name, match_.away_team_name
                    )
                })
                .unwrap_or_else(|| "No match row selected.".to_string()),
            _ => format!("{} detail placeholder open.", self.screen.title()),
        };
    }

    fn focus_next(&mut self) {
        if self.screen == Screen::Knockouts {
            self.focus_next_knockout_column();
            return;
        }

        self.focus_pane = match (self.screen, self.focus_pane) {
            (Screen::Home, FocusPane::None) => FocusPane::Content,
            (Screen::Home, FocusPane::Content | FocusPane::Detail) => FocusPane::None,
            (Screen::Standings, FocusPane::None) => FocusPane::Content,
            (Screen::Standings, FocusPane::Content | FocusPane::Detail) => FocusPane::None,
            (_, FocusPane::None) => FocusPane::Content,
            (_, FocusPane::Content) => FocusPane::Detail,
            (_, FocusPane::Detail) => FocusPane::None,
        };
        self.message = format!("Focus: {}.", self.focus_pane.label());
    }

    fn focus_previous(&mut self) {
        if self.screen == Screen::Knockouts {
            self.focus_previous_knockout_column();
            return;
        }

        self.focus_pane = match (self.screen, self.focus_pane) {
            (Screen::Home, FocusPane::None) => FocusPane::Content,
            (Screen::Home, FocusPane::Content | FocusPane::Detail) => FocusPane::None,
            (Screen::Standings, FocusPane::None) => FocusPane::Content,
            (Screen::Standings, FocusPane::Content | FocusPane::Detail) => FocusPane::None,
            (_, FocusPane::None) => FocusPane::Detail,
            (_, FocusPane::Detail) => FocusPane::Content,
            (_, FocusPane::Content) => FocusPane::None,
        };
        self.message = format!("Focus: {}.", self.focus_pane.label());
    }

    fn focus_next_knockout_column(&mut self) {
        let count = self.knockout_column_count();
        match self.focus_pane {
            FocusPane::None | FocusPane::Detail => {
                self.focus_pane = FocusPane::Content;
                self.knockout_column_cursor =
                    self.knockout_column_cursor.min(count.saturating_sub(1));
            }
            FocusPane::Content if self.knockout_column_cursor + 1 < count => {
                self.knockout_column_cursor += 1;
                self.knockout_match_cursor = 0;
            }
            FocusPane::Content => {
                self.focus_pane = FocusPane::None;
            }
        }
        self.scroll_to_focused_knockout();
        self.message = self.knockout_focus_message();
    }

    fn focus_previous_knockout_column(&mut self) {
        let count = self.knockout_column_count();
        match self.focus_pane {
            FocusPane::None | FocusPane::Detail => {
                self.focus_pane = FocusPane::Content;
                self.knockout_column_cursor = count.saturating_sub(1);
            }
            FocusPane::Content if self.knockout_column_cursor > 0 => {
                self.knockout_column_cursor -= 1;
                self.knockout_match_cursor = 0;
            }
            FocusPane::Content => {
                self.focus_pane = FocusPane::None;
            }
        }
        self.scroll_to_focused_knockout();
        self.message = self.knockout_focus_message();
    }

    fn knockout_focus_message(&self) -> String {
        if self.focus_pane != FocusPane::Content {
            return "Focus: none.".to_string();
        }

        let columns = self.visible_knockout_columns();
        let Some(column) = columns.get(self.knockout_column_cursor()) else {
            return "Focus: content.".to_string();
        };

        format!("Focused KO column: {}. Use arrows to scroll.", column.label)
    }

    fn scroll_up(&mut self, max: u16, visible_lines: u16) {
        match self.focus_pane {
            FocusPane::None => {
                self.message = "Focus a panel with Tab before scrolling.".to_string();
            }
            FocusPane::Content => {
                if matches!(self.screen, Screen::Countries) && self.visible_team_count() > 0 {
                    self.move_country_selection_up(max);
                    return;
                }
                if matches!(self.screen, Screen::Matches) && self.visible_match_count() > 0 {
                    self.move_match_selection_up(max, visible_lines);
                    return;
                }
                if matches!(self.screen, Screen::Standings)
                    && self.visible_standing_group_count() > 1
                {
                    self.move_standing_group_selection_up(max, visible_lines);
                    return;
                }
                let scroll = &mut self.current_scroll_mut().content;
                *scroll = (*scroll).min(max);
                *scroll = scroll.saturating_sub(1);
                self.message = format!("Content scroll: {}.", *scroll);
            }
            FocusPane::Detail => {
                let scroll = &mut self.current_scroll_mut().detail;
                *scroll = (*scroll).min(max);
                *scroll = scroll.saturating_sub(1);
                self.message = format!("Detail scroll: {}.", *scroll);
            }
        }
    }

    fn scroll_down(&mut self, max: u16, visible_lines: u16) {
        match self.focus_pane {
            FocusPane::None => {
                self.message = "Focus a panel with Tab before scrolling.".to_string();
            }
            FocusPane::Content => {
                if matches!(self.screen, Screen::Countries) && self.visible_team_count() > 0 {
                    self.move_country_selection_down(max);
                    return;
                }
                if matches!(self.screen, Screen::Matches) && self.visible_match_count() > 0 {
                    self.move_match_selection_down(max, visible_lines);
                    return;
                }
                if matches!(self.screen, Screen::Standings)
                    && self.visible_standing_group_count() > 1
                {
                    self.move_standing_group_selection_down(max, visible_lines);
                    return;
                }
                let scroll = &mut self.current_scroll_mut().content;
                *scroll = scroll.saturating_add(1).min(max);
                self.message = format!("Content scroll: {}.", *scroll);
            }
            FocusPane::Detail => {
                let scroll = &mut self.current_scroll_mut().detail;
                *scroll = scroll.saturating_add(1).min(max);
                self.message = format!("Detail scroll: {}.", *scroll);
            }
        }
    }

    fn scroll_left(&mut self, max: u16) {
        match self.focus_pane {
            FocusPane::None => {
                self.message = "Focus a panel with Tab before panning.".to_string();
            }
            FocusPane::Content => {
                let is_knockouts = self.screen == Screen::Knockouts;
                let scroll = &mut self.current_scroll_mut().content_x;
                let current = (*scroll).min(max);
                *scroll = if is_knockouts {
                    previous_knockout_column_scroll(current)
                } else {
                    current.saturating_sub(DEFAULT_HORIZONTAL_SCROLL_STEP)
                };
                self.message = format!("Content pan: {}.", *scroll);
            }
            FocusPane::Detail => {
                let scroll = &mut self.current_scroll_mut().detail_x;
                *scroll = (*scroll)
                    .min(max)
                    .saturating_sub(DEFAULT_HORIZONTAL_SCROLL_STEP);
                self.message = format!("Detail pan: {}.", *scroll);
            }
        }
    }

    fn scroll_right(&mut self, max: u16) {
        match self.focus_pane {
            FocusPane::None => {
                self.message = "Focus a panel with Tab before panning.".to_string();
            }
            FocusPane::Content => {
                let is_knockouts = self.screen == Screen::Knockouts;
                let scroll = &mut self.current_scroll_mut().content_x;
                let current = (*scroll).min(max);
                *scroll = if is_knockouts {
                    next_knockout_column_scroll(current, max)
                } else {
                    current
                        .saturating_add(DEFAULT_HORIZONTAL_SCROLL_STEP)
                        .min(max)
                };
                self.message = format!("Content pan: {}.", *scroll);
            }
            FocusPane::Detail => {
                let scroll = &mut self.current_scroll_mut().detail_x;
                *scroll = scroll
                    .saturating_add(DEFAULT_HORIZONTAL_SCROLL_STEP)
                    .min(max);
                self.message = format!("Detail pan: {}.", *scroll);
            }
        }
    }

    fn toggle_country_filter(&mut self, filter: CountriesFilter) {
        if !matches!(self.screen, Screen::Countries) {
            self.message = "Country filters are available on Countries.".to_string();
            return;
        }

        if filter.is_all() {
            self.country_filters.clear();
        } else if let Some(index) = self
            .country_filters
            .iter()
            .position(|selected| *selected == filter)
        {
            self.country_filters.remove(index);
        } else {
            self.country_filters.push(filter);
            self.country_filters.sort_by_key(|filter| filter.index());
        }

        if self.country_filters.len() == CountriesFilter::ALL.len() - 1 {
            self.country_filters.clear();
        }

        self.reset_country_selection();
        self.message = format!("Countries filter: {}.", self.country_filter_label());
    }

    fn select_matches_filter(&mut self, filter: MatchesFilter) {
        if !matches!(self.screen, Screen::Matches) {
            self.message = "Date filters are available on Matches.".to_string();
            return;
        }

        self.matches_filter = filter;
        self.reset_match_selection();
        self.message = format!("Match date filter: {}.", self.matches_filter.label());
    }

    fn toggle_match_group(&mut self, group: StandingsFilter) {
        if !matches!(self.screen, Screen::Matches) {
            self.message = "Group filters are available on Matches.".to_string();
            return;
        }

        if group.is_all() {
            self.match_groups.clear();
        } else if let Some(index) = self
            .match_groups
            .iter()
            .position(|selected| *selected == group)
        {
            self.match_groups.remove(index);
        } else {
            self.match_groups.push(group);
            self.match_groups.sort_by_key(|group| group.index());
        }

        self.reset_match_selection();
        self.message = format!("Match groups: {}.", self.match_group_label());
    }

    fn toggle_standing_group(&mut self, group: StandingsFilter) {
        if !matches!(self.screen, Screen::Standings) {
            self.message = "Group filters are available on Standings.".to_string();
            return;
        }

        if group.is_all() {
            self.standing_groups.clear();
        } else if let Some(index) = self
            .standing_groups
            .iter()
            .position(|selected| *selected == group)
        {
            self.standing_groups.remove(index);
        } else {
            self.standing_groups.push(group);
            self.standing_groups.sort_by_key(|group| group.index());
        }

        self.reset_standing_group_selection();
        self.message = format!("Standings groups: {}.", self.standing_group_label());
    }

    fn toggle_knockout_round(&mut self, round: KnockoutRoundFilter) {
        if !matches!(self.screen, Screen::Knockouts) {
            self.message = "Round filters are available on Knock-outs.".to_string();
            return;
        }

        if round.is_all() {
            self.knockout_rounds.clear();
        } else if let Some(index) = self
            .knockout_rounds
            .iter()
            .position(|selected| *selected == round)
        {
            self.knockout_rounds.remove(index);
        } else {
            self.knockout_rounds.push(round);
            self.knockout_rounds.sort_by_key(|round| round.index());
        }

        self.reset_knockout_selection();
        self.message = format!("Knock-out rounds: {}.", self.knockout_round_label());
    }

    fn select_stats_tab(&mut self, tab: StatsTab) {
        if !matches!(self.screen, Screen::Stats) {
            self.message = "Stats tabs are available on Stats.".to_string();
            return;
        }

        self.stats_tab = tab;
        self.message = format!("Stats tab: {}.", self.stats_tab.label());
    }

    fn toggle_timeline_filter(&mut self, filter: TimelineFilter) {
        if !matches!(self.screen, Screen::Matches) || self.focus_pane != FocusPane::Detail {
            self.message = "Timeline filters are available in focused match details.".to_string();
            return;
        }

        if let Some(index) = self
            .timeline_filters
            .iter()
            .position(|selected| *selected == filter)
        {
            self.timeline_filters.remove(index);
        } else {
            self.timeline_filters.push(filter);
            self.timeline_filters
                .sort_by_key(|selected| selected.index());
        }

        self.scroll[Screen::Matches.index()].detail = 0;
        self.message = format!("Timeline filters: {}.", self.timeline_filter_label());
    }

    fn toggle_sort(&mut self) {
        match self.screen {
            Screen::Countries => {
                self.countries_sort = self.countries_sort.toggle();
                self.clamp_country_cursor();
                self.message = format!("Countries sort: {}.", self.countries_sort.label());
            }
            Screen::Standings => {
                self.standings_sort = self.standings_sort.toggle();
                self.reset_standing_group_selection();
                self.message = format!("Standings sort: {}.", self.standings_sort.label());
            }
            _ => {
                self.message = "Sort is available on Countries and Standings.".to_string();
            }
        }
    }

    fn open_selected_standing_group_matches(&mut self) {
        if !matches!(self.screen, Screen::Standings) {
            self.message = "Focused group matches are available on Standings.".to_string();
            return;
        }

        let Some(group_id) = self.selected_standing_group_id() else {
            self.message = "No standings group selected.".to_string();
            return;
        };
        let Some(group) = standings_filter_for_group_id(&group_id) else {
            self.message = "Selected standings group is unknown.".to_string();
            return;
        };

        self.screen = Screen::Matches;
        self.input_mode = InputMode::Normal;
        self.help_open = false;
        self.detail_open = false;
        self.focus_pane = FocusPane::Content;
        self.matches_filter = MatchesFilter::All;
        self.match_groups = vec![group];
        self.reset_match_selection();
        self.message = format!("Matches filtered to {}.", group.label());
    }

    fn start_search(&mut self) {
        // if matches!(self.screen, Screen::Home | Screen::Facts) {
        //     self.message = format!("{} search is not implemented yet.", self.screen.title());
        //     return;
        // }

        self.input_mode = InputMode::Search;
        self.help_open = false;
        self.message = "Search mode. Type to filter, Enter to apply, Esc to close.".to_string();
    }

    fn search_input(&mut self, character: char) {
        if !character.is_control() {
            self.search_query.push(character);
            self.reset_country_selection();
            self.reset_match_selection();
            self.reset_standing_group_selection();
            self.reset_knockout_selection();
        }
    }

    fn search_backspace(&mut self) {
        self.search_query.pop();
        self.reset_country_selection();
        self.reset_match_selection();
        self.reset_standing_group_selection();
        self.reset_knockout_selection();
    }

    fn submit_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.reset_country_selection();
        self.reset_match_selection();
        self.reset_standing_group_selection();
        self.reset_knockout_selection();
        if self.search_query.is_empty() {
            self.message = "Search cleared.".to_string();
        } else {
            self.message = format!("Search filter set to '{}'.", self.search_query);
        }
    }

    fn toggle_selected_country_favorite(&mut self) {
        if !matches!(self.screen, Screen::Countries) {
            self.message = "Favorite country actions are available on Countries.".to_string();
            return;
        }

        let Some((team_id, team_name, was_favorite)) = self.selected_country_action() else {
            self.message = "No country selected.".to_string();
            return;
        };

        if let Some(team) = self
            .snapshot
            .teams
            .iter_mut()
            .find(|team| team.id == team_id)
        {
            team.favorite = !was_favorite;
        }

        let state = if was_favorite { "Removed" } else { "Added" };
        self.message = format!("{state} favorite: {team_name}.");
    }

    fn visible_team_count(&self) -> usize {
        self.visible_teams().len()
    }

    fn clamp_country_cursor(&mut self) {
        let count = self.visible_team_count();
        if count == 0 {
            self.country_cursor = 0;
        } else {
            self.country_cursor = self.country_cursor.min(count - 1);
        }
    }

    fn reset_country_selection(&mut self) {
        self.country_cursor = 0;
        self.scroll[Screen::Countries.index()].content = 0;
        self.clamp_country_cursor();
    }

    fn move_country_selection_up(&mut self, max_scroll: u16) {
        self.clamp_country_cursor();
        self.country_cursor = self.country_cursor.saturating_sub(1);
        self.current_scroll_mut().content = (self.country_cursor as u16).min(max_scroll);
        self.message = self
            .selected_country()
            .map(|team| format!("Selected country: {}.", team.name))
            .unwrap_or_else(|| "No country selected.".to_string());
    }

    fn move_country_selection_down(&mut self, max_scroll: u16) {
        let count = self.visible_team_count();
        if count == 0 {
            self.country_cursor = 0;
            self.message = "No country selected.".to_string();
            return;
        }

        self.country_cursor = (self.country_cursor + 1).min(count - 1);
        self.current_scroll_mut().content = (self.country_cursor as u16).min(max_scroll);
        self.message = self
            .selected_country()
            .map(|team| format!("Selected country: {}.", team.name))
            .unwrap_or_else(|| "No country selected.".to_string());
    }

    fn clamp_match_cursor(&mut self) {
        let count = self.visible_match_count();
        if count == 0 {
            self.match_cursor = 0;
        } else {
            self.match_cursor = self.match_cursor.min(count - 1);
        }
    }

    fn reset_match_selection(&mut self) {
        self.match_cursor = 0;
        self.scroll[Screen::Matches.index()].content = 0;
        self.reset_match_detail_view();
        self.clamp_match_cursor();
    }

    fn reset_match_detail_view(&mut self) {
        self.timeline_filters.clear();
        self.scroll[Screen::Matches.index()].detail = 0;
    }

    fn move_match_selection_up(&mut self, max_scroll: u16, visible_lines: u16) {
        self.clamp_match_cursor();
        let previous_cursor = self.match_cursor;
        self.match_cursor = self.match_cursor.saturating_sub(1);
        if self.match_cursor != previous_cursor {
            self.reset_match_detail_view();
        }
        self.current_scroll_mut().content = self.selected_match_scroll(max_scroll, visible_lines);
        self.message = self
            .selected_match()
            .map(|match_| {
                format!(
                    "Selected match: {} vs {}.",
                    match_.home_team_name, match_.away_team_name
                )
            })
            .unwrap_or_else(|| "No match selected.".to_string());
    }

    fn move_match_selection_down(&mut self, max_scroll: u16, visible_lines: u16) {
        let count = self.visible_match_count();
        if count == 0 {
            self.match_cursor = 0;
            self.message = "No match selected.".to_string();
            return;
        }

        let previous_cursor = self.match_cursor;
        self.match_cursor = (self.match_cursor + 1).min(count - 1);
        if self.match_cursor != previous_cursor {
            self.reset_match_detail_view();
        }
        self.current_scroll_mut().content = self.selected_match_scroll(max_scroll, visible_lines);
        self.message = self
            .selected_match()
            .map(|match_| {
                format!(
                    "Selected match: {} vs {}.",
                    match_.home_team_name, match_.away_team_name
                )
            })
            .unwrap_or_else(|| "No match selected.".to_string());
    }

    fn selected_match_scroll(&self, max_scroll: u16, visible_lines: u16) -> u16 {
        let visible_row_index = if self.detail_open {
            0
        } else {
            self.match_cursor
        };
        let selected_line = MATCHES_CONTENT_PREAMBLE_LINES
            .saturating_add(u16::try_from(visible_row_index).unwrap_or(u16::MAX));
        selected_line
            .saturating_add(1)
            .saturating_sub(visible_lines.max(1))
            .min(max_scroll)
    }

    fn clamp_standing_group_cursor(&mut self) {
        let count = self.visible_standing_group_count();
        if count == 0 {
            self.standing_group_cursor = 0;
        } else {
            self.standing_group_cursor = self.standing_group_cursor.min(count - 1);
        }
    }

    fn reset_standing_group_selection(&mut self) {
        self.standing_group_cursor = 0;
        self.scroll[Screen::Standings.index()].content = 0;
        self.clamp_standing_group_cursor();
    }

    fn reset_knockout_selection(&mut self) {
        self.knockout_column_cursor = 0;
        self.knockout_match_cursor = 0;
        self.scroll[Screen::Knockouts.index()].content = 0;
        self.scroll[Screen::Knockouts.index()].content_x = 0;
    }

    fn move_standing_group_selection_up(&mut self, max_scroll: u16, visible_lines: u16) {
        self.clamp_standing_group_cursor();
        self.standing_group_cursor = self.standing_group_cursor.saturating_sub(1);
        self.scroll[Screen::Standings.index()].content =
            self.selected_standing_group_scroll(max_scroll, visible_lines);
        self.message = self
            .selected_standing_group_id()
            .and_then(|group_id| standings_filter_for_group_id(&group_id))
            .map(|group| format!("Selected standings group: {}.", group.label()))
            .unwrap_or_else(|| "No standings group selected.".to_string());
    }

    fn move_standing_group_selection_down(&mut self, max_scroll: u16, visible_lines: u16) {
        let count = self.visible_standing_group_count();
        if count == 0 {
            self.standing_group_cursor = 0;
            self.message = "No standings group selected.".to_string();
            return;
        }

        self.standing_group_cursor = (self.standing_group_cursor + 1).min(count - 1);
        self.scroll[Screen::Standings.index()].content =
            self.selected_standing_group_scroll(max_scroll, visible_lines);
        self.message = self
            .selected_standing_group_id()
            .and_then(|group_id| standings_filter_for_group_id(&group_id))
            .map(|group| format!("Selected standings group: {}.", group.label()))
            .unwrap_or_else(|| "No standings group selected.".to_string());
    }

    fn selected_standing_group_scroll(&self, max_scroll: u16, visible_lines: u16) -> u16 {
        let Some(selected_group_id) = self.selected_standing_group_id() else {
            return 0;
        };

        let mut current_group_id: Option<GroupId> = None;
        let mut line = STANDINGS_CONTENT_PREAMBLE_LINES;
        let mut selected_group_start = 0;
        let mut selected_group_rows = 0;
        let mut in_selected_group = false;
        for row in self.visible_standings() {
            if current_group_id.as_ref() != Some(&row.group_id) {
                if in_selected_group {
                    return standing_group_scroll_for_block(
                        selected_group_start,
                        selected_group_rows,
                        max_scroll,
                        visible_lines,
                    );
                }

                if current_group_id.is_some() {
                    line += 1;
                }

                current_group_id = Some(row.group_id.clone());
                in_selected_group = row.group_id == selected_group_id;
                if in_selected_group {
                    selected_group_start = line;
                    selected_group_rows = 0;
                }
                line += 2;
            }

            if in_selected_group {
                selected_group_rows += 1;
            }
            line += 1;
        }

        if in_selected_group {
            standing_group_scroll_for_block(
                selected_group_start,
                selected_group_rows,
                max_scroll,
                visible_lines,
            )
        } else {
            0
        }
    }

    fn scroll_to_focused_knockout(&mut self) {
        if self.screen != Screen::Knockouts || self.focus_pane != FocusPane::Content {
            return;
        }

        self.knockout_column_cursor = self
            .knockout_column_cursor
            .min(self.knockout_column_count().saturating_sub(1));
        self.knockout_match_cursor = self.knockout_match_cursor.min(
            self.knockout_selected_column_match_count()
                .saturating_sub(1),
        );

        let selected_column = self
            .visible_knockout_columns()
            .get(self.knockout_column_cursor)
            .copied()
            .unwrap_or_else(|| KnockoutColumn::single(KnockoutRoundFilter::RoundOf32));

        self.scroll[Screen::Knockouts.index()].content_x =
            (self.knockout_column_cursor as u16).saturating_mul(KNOCKOUT_COLUMN_WIDTH);
        self.scroll[Screen::Knockouts.index()].content = selected_column
            .selected_match_y(self.knockout_match_cursor)
            .saturating_add(KNOCKOUT_CONTENT_PREAMBLE_LINES)
            .saturating_sub(4);
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    pub fn quit_confirm_open(&self) -> bool {
        self.input_mode == InputMode::QuitConfirm
    }
}

fn contains_query(value: &str, query: &str) -> bool {
    value.to_lowercase().contains(query)
}

#[derive(Clone, Copy)]
struct KnockoutColumn {
    label: &'static str,
    kind: KnockoutColumnKind,
    match_count: usize,
}

impl KnockoutColumn {
    fn new(label: &'static str, kind: KnockoutColumnKind, match_count: usize) -> Self {
        Self {
            label,
            kind,
            match_count,
        }
    }

    fn single(round: KnockoutRoundFilter) -> Self {
        match round {
            KnockoutRoundFilter::All => Self::new("Round of 32", KnockoutColumnKind::RoundOf32, 0),
            KnockoutRoundFilter::RoundOf32 => {
                Self::new("Round of 32", KnockoutColumnKind::RoundOf32, 16)
            }
            KnockoutRoundFilter::RoundOf16 => {
                Self::new("Round of 16", KnockoutColumnKind::RoundOf16, 8)
            }
            KnockoutRoundFilter::QuarterFinal => {
                Self::new("quarter-finals", KnockoutColumnKind::QuarterFinal, 4)
            }
            KnockoutRoundFilter::SemiFinal => {
                Self::new("semi-finals", KnockoutColumnKind::SemiFinal, 2)
            }
            KnockoutRoundFilter::ThirdPlace => {
                Self::new("third place", KnockoutColumnKind::Center, 1)
            }
            KnockoutRoundFilter::Final => Self::new("final", KnockoutColumnKind::Center, 1),
        }
    }

    fn selected_match_y(self, match_cursor: usize) -> u16 {
        match self.kind {
            KnockoutColumnKind::RoundOf32 => match_cursor as u16 * 4,
            KnockoutColumnKind::RoundOf16 => match_cursor as u16 * 8 + 2,
            KnockoutColumnKind::QuarterFinal => match_cursor as u16 * 16 + 6,
            KnockoutColumnKind::SemiFinal => 14,
            KnockoutColumnKind::Center => {
                if match_cursor == 0 {
                    12
                } else {
                    20
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum KnockoutColumnKind {
    RoundOf32,
    RoundOf16,
    QuarterFinal,
    SemiFinal,
    Center,
}

fn standing_group_scroll_for_block(
    group_start: u16,
    group_rows: u16,
    max_scroll: u16,
    visible_lines: u16,
) -> u16 {
    if group_start == STANDINGS_CONTENT_PREAMBLE_LINES {
        return 0;
    }

    let visible_lines = visible_lines.max(1);
    let group_block_height = group_rows.saturating_add(2);
    group_start
        .saturating_add(group_block_height.min(visible_lines))
        .saturating_sub(visible_lines)
        .min(max_scroll)
}

fn standing_group_sort_key(group_id: &GroupId) -> usize {
    Group::ALL
        .iter()
        .position(|group| group.id() == *group_id)
        .unwrap_or(usize::MAX)
}

fn standings_filter_for_group_id(group_id: &GroupId) -> Option<StandingsFilter> {
    match Group::from_id(group_id.as_str()).ok()? {
        Group::A => Some(StandingsFilter::GroupA),
        Group::B => Some(StandingsFilter::GroupB),
        Group::C => Some(StandingsFilter::GroupC),
        Group::D => Some(StandingsFilter::GroupD),
        Group::E => Some(StandingsFilter::GroupE),
        Group::F => Some(StandingsFilter::GroupF),
        Group::G => Some(StandingsFilter::GroupG),
        Group::H => Some(StandingsFilter::GroupH),
        Group::I => Some(StandingsFilter::GroupI),
        Group::J => Some(StandingsFilter::GroupJ),
        Group::K => Some(StandingsFilter::GroupK),
        Group::L => Some(StandingsFilter::GroupL),
    }
}

fn matches_with_all_standings_resources() -> Vec<ResourceKey> {
    let mut resources = vec![ResourceKey::Matches];
    resources.extend(ResourceKey::all_standings_groups());
    resources
}

fn match_needs_automatic_refresh(match_: &Match, now: Timestamp) -> bool {
    if matches!(
        match_.status,
        MatchStatus::Live | MatchStatus::ExtraTime | MatchStatus::Penalties
    ) {
        return true;
    }

    let seconds_until_start = match_.utc_start.duration_since(now).as_secs_f64();
    if seconds_until_start >= 0.0 && seconds_until_start <= 2.0 * 60.0 * 60.0 {
        return true;
    }

    let seconds_since_start = now.duration_since(match_.utc_start).as_secs_f64();
    seconds_since_start >= 0.0 && seconds_since_start <= 4.0 * 60.0 * 60.0
}

fn previous_knockout_column_scroll(current: u16) -> u16 {
    if current == 0 {
        return 0;
    }

    let remainder = current % KNOCKOUT_COLUMN_WIDTH;
    if remainder == 0 {
        current.saturating_sub(KNOCKOUT_COLUMN_WIDTH)
    } else {
        current.saturating_sub(remainder)
    }
}

fn next_knockout_column_scroll(current: u16, max: u16) -> u16 {
    let next = current
        .saturating_add(KNOCKOUT_COLUMN_WIDTH)
        .saturating_sub(current % KNOCKOUT_COLUMN_WIDTH);

    next.min(max)
}

fn standing_fair_play(row: &StandingRow) -> i16 {
    row.fair_play.unwrap_or(i16::MIN)
}

fn team_fifa_rank(app: &App, team_id: &TeamId) -> u16 {
    app.snapshot
        .teams
        .iter()
        .find(|team| &team.id == team_id)
        .and_then(|team| team.fifa_rank)
        .unwrap_or(u16::MAX)
}

fn fallback_code(value: &str) -> String {
    let code = value
        .chars()
        .filter(|character| character.is_ascii_alphabetic())
        .take(3)
        .collect::<String>()
        .to_ascii_uppercase();

    if code.is_empty() {
        "TBD".to_string()
    } else {
        code
    }
}

fn relative_time_label(target: Timestamp, now: Timestamp) -> String {
    let seconds = target.duration_since(now).as_secs_f64().round() as i64;
    let direction = if seconds >= 0 { "in" } else { "ago" };
    let seconds = seconds.unsigned_abs();
    let label = if seconds < 60 {
        format!("{}s", seconds.max(1))
    } else if seconds < 3_600 {
        format!("{}m", (seconds / 60).max(1))
    } else if seconds < 86_400 {
        format!("{}h{}m", (seconds / 3_600).max(1), (seconds % 3600) / 60)
    } else {
        let days = (seconds / 86_400).max(1);
        let hours = (seconds % 86_400) / 3_600;
        if hours > 0 {
            format!("{days}d{hours}h")
        } else {
            format!("{days}d")
        }
    };

    if direction == "in" {
        format!("in {label}")
    } else {
        format!("{label} ago")
    }
}

fn match_is_future(match_: &Match) -> bool {
    matches!(match_.status, MatchStatus::Scheduled) && match_.utc_start > Timestamp::now()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WORLD_CUP_2026;
    use crate::domain::{Confederation, MatchId, MatchStatus, StageId, TeamId, TimelineEvent};

    #[test]
    fn navigation_command_changes_screen() {
        let mut app = App::new(WORLD_CUP_2026);

        app.handle_command(AppCommand::Navigate(Screen::Matches));

        assert_eq!(app.screen(), Screen::Matches);
        assert_eq!(app.input_mode(), InputMode::Normal);
    }

    #[test]
    fn country_filters_can_select_multiple_associations() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        app.handle_command(AppCommand::ToggleCountryFilter(CountriesFilter::Afc));
        app.handle_command(AppCommand::ToggleCountryFilter(CountriesFilter::Conmebol));
        assert_eq!(
            app.country_filters(),
            &[CountriesFilter::Afc, CountriesFilter::Conmebol]
        );
        assert_eq!(app.country_filter_label(), "AFC, CONMEBOL");

        app.handle_command(AppCommand::ToggleCountryFilter(CountriesFilter::All));
        assert!(app.country_filters().is_empty());
        assert_eq!(app.country_filter_label(), "All");
    }

    #[test]
    fn country_filters_collapse_to_all_when_every_association_is_selected() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        for filter in CountriesFilter::ALL
            .iter()
            .copied()
            .filter(|filter| !filter.is_all())
        {
            app.handle_command(AppCommand::ToggleCountryFilter(filter));
        }

        assert!(app.country_filters().is_empty());
        assert_eq!(app.country_filter_label(), "All");
    }

    #[test]
    fn standing_groups_can_select_multiple_groups() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Standings));

        app.handle_command(AppCommand::ToggleStandingGroup(StandingsFilter::GroupA));
        app.handle_command(AppCommand::ToggleStandingGroup(StandingsFilter::GroupC));
        assert_eq!(
            app.standing_groups(),
            &[StandingsFilter::GroupA, StandingsFilter::GroupC]
        );
        assert_eq!(app.standing_group_label(), "Group A, Group C");

        app.handle_command(AppCommand::ToggleStandingGroup(StandingsFilter::AllGroups));
        assert!(app.standing_groups().is_empty());
        assert_eq!(app.standing_group_label(), "All groups");
    }

    #[test]
    fn knockout_rounds_can_select_multiple_rounds() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Knockouts));

        app.handle_command(AppCommand::ToggleKnockoutRound(
            KnockoutRoundFilter::RoundOf32,
        ));
        app.handle_command(AppCommand::ToggleKnockoutRound(KnockoutRoundFilter::Final));
        assert_eq!(
            app.knockout_rounds(),
            &[KnockoutRoundFilter::RoundOf32, KnockoutRoundFilter::Final]
        );
        assert_eq!(app.knockout_round_label(), "Round of 32, Final");

        app.handle_command(AppCommand::ToggleKnockoutRound(KnockoutRoundFilter::All));
        assert!(app.knockout_rounds().is_empty());
        assert_eq!(app.knockout_round_label(), "All rounds");
    }

    #[test]
    fn visible_standings_filter_and_sort_by_group() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            standings: vec![
                sample_standing(Group::C, "43920", "Japan", 2),
                sample_standing(Group::A, "43911", "Mexico", 2),
                sample_standing(Group::C, "43921", "Switzerland", 1),
                sample_standing(Group::A, "43922", "Argentina", 1),
            ],
            ..AppSnapshot::default()
        });

        assert_eq!(
            app.visible_standings()
                .iter()
                .map(|row| (
                    row.group_name.as_str(),
                    row.position,
                    row.team_name.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("Group A", 1, "Argentina"),
                ("Group A", 2, "Mexico"),
                ("Group C", 1, "Switzerland"),
                ("Group C", 2, "Japan"),
            ]
        );

        app.handle_command(AppCommand::Navigate(Screen::Standings));
        app.handle_command(AppCommand::ToggleSort);
        assert_eq!(
            app.visible_standings()
                .iter()
                .map(|row| (
                    row.group_name.as_str(),
                    row.position,
                    row.team_name.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("Group C", 1, "Switzerland"),
                ("Group C", 2, "Japan"),
                ("Group A", 1, "Argentina"),
                ("Group A", 2, "Mexico"),
            ]
        );

        app.handle_command(AppCommand::ToggleStandingGroup(StandingsFilter::GroupA));
        assert_eq!(
            app.visible_standings()
                .iter()
                .map(|row| row.team_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Argentina", "Mexico"]
        );
    }

    #[test]
    fn focused_standing_group_opens_matching_matches_filter() {
        let mut group_c_match =
            sample_match("4002", 2, "Japan", "Switzerland", "2026-06-12T19:00:00Z");
        group_c_match.group_id = Some(Group::C.id());
        group_c_match.group_name = Some("Group C".to_string());

        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            standings: vec![
                sample_standing(Group::A, "43911", "Mexico", 1),
                sample_standing(Group::C, "43920", "Japan", 1),
            ],
            matches: vec![
                sample_match("4001", 1, "Mexico", "Argentina", "2026-06-11T19:00:00Z"),
                group_c_match,
            ],
            ..AppSnapshot::default()
        });

        app.handle_command(AppCommand::Navigate(Screen::Standings));
        app.handle_command(AppCommand::FocusNext);
        app.handle_command(AppCommand::ScrollDown {
            max: 100,
            visible_lines: 20,
        });
        app.handle_command(AppCommand::OpenSelectedStandingGroupMatches);

        assert_eq!(app.screen(), Screen::Matches);
        assert_eq!(app.focus_pane(), FocusPane::Content);
        assert_eq!(app.match_group_label(), "Group C");
        assert_eq!(
            app.visible_matches()
                .iter()
                .map(|match_| match_.home_team_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Japan"]
        );
    }

    #[test]
    fn standing_group_selection_scroll_returns_to_helper_lines_at_top() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            standings: vec![
                sample_standing(Group::A, "a1", "Mexico", 1),
                sample_standing(Group::A, "a2", "Canada", 2),
                sample_standing(Group::A, "a3", "Switzerland", 3),
                sample_standing(Group::A, "a4", "Qatar", 4),
                sample_standing(Group::B, "b1", "Argentina", 1),
                sample_standing(Group::B, "b2", "Japan", 2),
                sample_standing(Group::B, "b3", "France", 3),
                sample_standing(Group::B, "b4", "Egypt", 4),
                sample_standing(Group::C, "c1", "Brazil", 1),
                sample_standing(Group::C, "c2", "Spain", 2),
                sample_standing(Group::C, "c3", "Uruguay", 3),
                sample_standing(Group::C, "c4", "Ghana", 4),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Standings));
        app.handle_command(AppCommand::FocusNext);

        app.handle_command(AppCommand::ScrollDown {
            max: 18,
            visible_lines: 6,
        });
        app.handle_command(AppCommand::ScrollDown {
            max: 18,
            visible_lines: 6,
        });
        assert_eq!(app.selected_standing_group_id(), Some(Group::C.id()));
        assert_eq!(app.content_scroll(), 18);

        app.handle_command(AppCommand::ScrollUp {
            max: 18,
            visible_lines: 6,
        });
        app.handle_command(AppCommand::ScrollUp {
            max: 18,
            visible_lines: 6,
        });
        assert_eq!(app.selected_standing_group_id(), Some(Group::A.id()));
        assert_eq!(app.content_scroll(), 0);
    }

    #[test]
    fn best_third_advancement_uses_fifa_rank_as_final_tiebreaker() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![
                sample_ranked_team("t1", "One", "ONE", 1),
                sample_ranked_team("t2", "Two", "TWO", 2),
                sample_ranked_team("t3", "Three", "THR", 3),
                sample_ranked_team("t4", "Four", "FOU", 4),
                sample_ranked_team("t5", "Five", "FIV", 80),
                sample_ranked_team("t6", "Six", "SIX", 70),
                sample_ranked_team("t7", "Seven", "SEV", 60),
                sample_ranked_team("t8", "Eight", "EIG", 10),
                sample_ranked_team("t9", "Nine", "NIN", 90),
            ],
            standings: vec![
                sample_third_standing(Group::A, "t1", "One", 6, 0, 1, -1),
                sample_third_standing(Group::B, "t2", "Two", 5, 0, 1, -1),
                sample_third_standing(Group::C, "t3", "Three", 4, 2, 3, -1),
                sample_third_standing(Group::D, "t4", "Four", 3, 2, 3, -1),
                sample_third_standing(Group::E, "t5", "Five", 3, 1, 5, -1),
                sample_third_standing(Group::F, "t6", "Six", 3, 1, 5, -1),
                sample_third_standing(Group::G, "t7", "Seven", 3, 1, 5, -1),
                sample_third_standing(Group::H, "t8", "Eight", 3, 1, 5, -1),
                sample_third_standing(Group::I, "t9", "Nine", 3, 1, 5, -1),
            ],
            ..AppSnapshot::default()
        });

        let eight = app
            .visible_standings()
            .into_iter()
            .find(|row| row.team_name == "Eight")
            .expect("eighth-best third");
        assert!(app.standing_row_is_advancing(eight));

        let nine = app
            .visible_standings()
            .into_iter()
            .find(|row| row.team_name == "Nine")
            .expect("ninth-best third");
        assert!(!app.standing_row_is_advancing(nine));
    }

    #[test]
    fn favorite_filter_is_global() {
        let mut app = App::new(WORLD_CUP_2026);

        app.handle_command(AppCommand::ToggleFavoriteOnly);
        assert!(app.favorite_only());
    }

    #[test]
    fn space_command_toggles_key_scope() {
        let mut app = App::new(WORLD_CUP_2026);

        app.handle_command(AppCommand::ToggleKeyScope);
        assert_eq!(app.key_scope(), KeyScope::Screen);

        app.handle_command(AppCommand::ToggleKeyScope);
        assert_eq!(app.key_scope(), KeyScope::Global);
    }

    #[test]
    fn focus_commands_cycle_panels() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        app.handle_command(AppCommand::FocusNext);
        assert_eq!(app.focus_pane(), FocusPane::Content);

        app.handle_command(AppCommand::FocusNext);
        assert_eq!(app.focus_pane(), FocusPane::Detail);

        app.handle_command(AppCommand::FocusNext);
        assert_eq!(app.focus_pane(), FocusPane::None);
    }

    #[test]
    fn scroll_commands_apply_to_focused_screen_panel() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        app.handle_command(AppCommand::FocusNext);
        app.handle_command(AppCommand::ScrollDown {
            max: 3,
            visible_lines: 5,
        });
        app.handle_command(AppCommand::ScrollDown {
            max: 3,
            visible_lines: 5,
        });
        assert_eq!(app.content_scroll(), 2);
        assert_eq!(app.detail_scroll(), 0);

        app.handle_command(AppCommand::Navigate(Screen::Matches));
        assert_eq!(app.content_scroll(), 0);

        app.handle_command(AppCommand::Navigate(Screen::Countries));
        assert_eq!(app.content_scroll(), 2);

        app.handle_command(AppCommand::FocusNext);
        app.handle_command(AppCommand::ScrollUp {
            max: 3,
            visible_lines: 5,
        });
        assert_eq!(app.content_scroll(), 1);
    }

    #[test]
    fn scroll_down_does_not_accumulate_beyond_max() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));
        app.handle_command(AppCommand::FocusNext);

        for _ in 0..5 {
            app.handle_command(AppCommand::ScrollDown {
                max: 2,
                visible_lines: 5,
            });
        }

        assert_eq!(app.content_scroll(), 2);

        app.handle_command(AppCommand::ScrollUp {
            max: 2,
            visible_lines: 5,
        });
        assert_eq!(app.content_scroll(), 1);
    }

    #[test]
    fn knockout_focus_cycles_columns_and_arrows_scroll_content() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Knockouts));

        app.handle_command(AppCommand::FocusNext);
        assert_eq!(app.knockout_column_cursor(), 0);
        assert_eq!(app.knockout_match_cursor(), 0);

        app.handle_command(AppCommand::FocusNext);
        assert_eq!(app.knockout_column_cursor(), 1);
        assert_eq!(app.knockout_match_cursor(), 0);
        assert!(app.content_horizontal_scroll() > 0);

        let focused_column = app.knockout_column_cursor();
        app.handle_command(AppCommand::ScrollRight { max: 80 });
        assert_eq!(app.knockout_column_cursor(), focused_column);
        assert_eq!(app.content_horizontal_scroll(), 54);
        app.handle_command(AppCommand::ScrollLeft { max: 80 });
        assert_eq!(app.knockout_column_cursor(), focused_column);
        assert_eq!(app.content_horizontal_scroll(), 27);

        let scroll_before = app.content_scroll();
        app.handle_command(AppCommand::ScrollDown {
            max: 40,
            visible_lines: 10,
        });
        assert_eq!(app.knockout_match_cursor(), 0);
        assert_eq!(app.content_scroll(), scroll_before + 1);

        app.handle_command(AppCommand::FocusPrevious);
        assert_eq!(app.knockout_column_cursor(), 0);
        assert_eq!(app.knockout_match_cursor(), 0);
    }

    #[test]
    fn refresh_resources_are_screen_specific() {
        let mut app = App::new(WORLD_CUP_2026);
        assert_eq!(app.refresh_resources().len(), 16);

        app.handle_command(AppCommand::Navigate(Screen::Countries));
        assert_eq!(
            app.refresh_resources(),
            vec![ResourceKey::Teams, ResourceKey::Stages]
        );

        app.handle_command(AppCommand::Navigate(Screen::Matches));
        assert_eq!(app.refresh_resources().len(), 13);
        assert_eq!(app.refresh_resources()[0], ResourceKey::Matches);

        app.handle_command(AppCommand::Navigate(Screen::Standings));
        assert_eq!(app.refresh_resources().len(), 12);

        app.handle_command(AppCommand::ToggleStandingGroup(StandingsFilter::GroupA));
        assert_eq!(
            app.refresh_resources(),
            vec![ResourceKey::StandingsGroup(Group::A.id())]
        );

        app.handle_command(AppCommand::Navigate(Screen::Knockouts));
        assert_eq!(app.refresh_resources().len(), 13);
        assert_eq!(app.refresh_resources()[0], ResourceKey::Matches);
    }

    #[test]
    fn refresh_events_update_source_status() {
        let mut app = App::new(WORLD_CUP_2026);

        app.handle_refresh_event(RefreshEvent::Started {
            resource: ResourceKey::Teams,
        });
        assert_eq!(app.source_status().state, SourceState::Refreshing);
        assert_eq!(app.source_status().pending_refreshes, 1);

        app.handle_refresh_event(RefreshEvent::Succeeded {
            resource: ResourceKey::Teams,
            at: "2026-06-18T00:00:00Z".to_string(),
        });
        assert_eq!(app.source_status().state, SourceState::Cached);
        assert_eq!(app.source_status().pending_refreshes, 0);
        assert_eq!(
            app.source_status().last_updated.as_deref(),
            Some("2026-06-18T00:00:00Z")
        );

        app.handle_refresh_event(RefreshEvent::Offline {
            resource: ResourceKey::Matches,
            error: "network unreachable".to_string(),
        });
        assert_eq!(app.source_status().state, SourceState::Offline);
    }

    #[test]
    fn sort_toggle_applies_to_countries_and_standings() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        app.handle_command(AppCommand::ToggleSort);
        assert_eq!(app.countries_sort(), SortOrder::Desc);
        assert_eq!(app.standings_sort(), SortOrder::Asc);

        app.handle_command(AppCommand::Navigate(Screen::Standings));
        app.handle_command(AppCommand::ToggleSort);
        assert_eq!(app.countries_sort(), SortOrder::Desc);
        assert_eq!(app.standings_sort(), SortOrder::Desc);
    }

    #[test]
    fn search_mode_collects_and_submits_text() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        app.handle_command(AppCommand::StartSearch);
        app.handle_command(AppCommand::SearchInput('a'));
        app.handle_command(AppCommand::SearchInput('r'));
        app.handle_command(AppCommand::SubmitSearch);

        assert_eq!(app.input_mode(), InputMode::Normal);
    }

    #[test]
    fn visible_teams_apply_filters_search_and_sort() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![
                sample_team("43922", "Argentina", "ARG", Confederation::Conmebol),
                sample_team("43911", "Mexico", "MEX", Confederation::Concacaf),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Countries));

        app.handle_command(AppCommand::ToggleCountryFilter(CountriesFilter::Conmebol));
        assert_eq!(app.visible_teams()[0].name, "Argentina");

        app.handle_command(AppCommand::StartSearch);
        app.handle_command(AppCommand::SearchInput('m'));
        app.handle_command(AppCommand::SearchInput('e'));
        app.handle_command(AppCommand::SearchInput('x'));
        app.handle_command(AppCommand::SubmitSearch);
        assert!(app.visible_teams().is_empty());
    }

    #[test]
    fn country_selection_moves_and_toggles_favorite() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![
                sample_team("43922", "Argentina", "ARG", Confederation::Conmebol),
                sample_team("43911", "Mexico", "MEX", Confederation::Concacaf),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Countries));
        app.handle_command(AppCommand::FocusNext);

        assert_eq!(
            app.selected_country_action().map(|(_, name, _)| name),
            Some("Argentina".to_string())
        );

        app.handle_command(AppCommand::ScrollDown {
            max: 10,
            visible_lines: 5,
        });
        assert_eq!(
            app.selected_country_action().map(|(_, name, _)| name),
            Some("Mexico".to_string())
        );

        app.handle_command(AppCommand::ToggleSelectedCountryFavorite);
        assert_eq!(
            app.selected_country_action()
                .map(|(_, _, favorite)| favorite),
            Some(true)
        );

        app.handle_command(AppCommand::ToggleCountryFilter(CountriesFilter::Conmebol));
        assert_eq!(app.content_scroll(), 0);
        assert_eq!(
            app.selected_country_action().map(|(_, name, _)| name),
            Some("Argentina".to_string())
        );
    }

    #[test]
    fn quit_requires_confirmation() {
        let mut app = App::new(WORLD_CUP_2026);

        app.handle_command(AppCommand::Quit);
        assert!(app.is_running());
        assert!(app.quit_confirm_open());

        app.handle_command(AppCommand::CloseOverlay);
        assert!(app.is_running());
        assert_eq!(app.input_mode(), InputMode::Normal);

        app.handle_command(AppCommand::Quit);
        app.handle_command(AppCommand::ConfirmQuit);
        assert!(!app.is_running());
    }

    #[test]
    fn match_selection_moves_with_focused_content() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            matches: vec![
                sample_match("4001", 1, "Argentina", "France", "2026-06-18T12:00:00Z"),
                sample_match("4002", 2, "Mexico", "Canada", "2026-06-19T12:00:00Z"),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Matches));
        app.handle_command(AppCommand::FocusNext);

        assert_eq!(app.selected_match_id(), Some(MatchId::from("4001")));

        app.handle_command(AppCommand::ScrollDown {
            max: 10,
            visible_lines: 5,
        });
        assert_eq!(app.selected_match_id(), Some(MatchId::from("4002")));

        app.handle_command(AppCommand::ToggleMatchGroup(StandingsFilter::GroupA));
        assert_eq!(app.content_scroll(), 0);
        assert_eq!(app.selected_match_id(), Some(MatchId::from("4001")));
    }

    #[test]
    fn match_selection_scroll_returns_to_helper_lines_at_top() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            matches: vec![
                sample_match("4001", 1, "Argentina", "France", "2026-06-18T12:00:00Z"),
                sample_match("4002", 2, "Mexico", "Canada", "2026-06-19T12:00:00Z"),
                sample_match("4003", 3, "Japan", "Spain", "2026-06-20T12:00:00Z"),
                sample_match("4004", 4, "Brazil", "Germany", "2026-06-21T12:00:00Z"),
                sample_match("4005", 5, "Uruguay", "Italy", "2026-06-22T12:00:00Z"),
                sample_match("4006", 6, "Qatar", "Canada", "2026-06-23T12:00:00Z"),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Matches));
        app.handle_command(AppCommand::FocusNext);

        for _ in 0..5 {
            app.handle_command(AppCommand::ScrollDown {
                max: 10,
                visible_lines: 6,
            });
        }
        assert!(app.content_scroll() > 0);

        for _ in 0..5 {
            app.handle_command(AppCommand::ScrollUp {
                max: 10,
                visible_lines: 6,
            });
        }
        assert_eq!(app.selected_match_id(), Some(MatchId::from("4001")));
        assert_eq!(app.content_scroll(), 0);
    }

    #[test]
    fn visible_matches_combine_date_and_group_filters() {
        let mut past_group_a =
            sample_match("4001", 1, "Argentina", "France", "2000-06-18T12:00:00Z");
        past_group_a.group_id = Some(Group::A.id());
        past_group_a.group_name = Some("Group A".to_string());

        let mut future_group_a =
            sample_match("4002", 2, "Mexico", "Canada", "2999-06-19T12:00:00Z");
        future_group_a.group_id = Some(Group::A.id());
        future_group_a.group_name = Some("Group A".to_string());

        let mut past_group_b =
            sample_match("4003", 3, "Japan", "Switzerland", "2000-06-20T12:00:00Z");
        past_group_b.group_id = Some(Group::B.id());
        past_group_b.group_name = Some("Group B".to_string());

        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            matches: vec![past_group_a, future_group_a, past_group_b],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Matches));

        app.handle_command(AppCommand::SelectMatchesFilter(MatchesFilter::Past));
        app.handle_command(AppCommand::ToggleMatchGroup(StandingsFilter::GroupA));

        assert_eq!(
            app.visible_matches()
                .iter()
                .map(|match_| match_.id.clone())
                .collect::<Vec<_>>(),
            vec![MatchId::from("4001")]
        );
    }

    #[test]
    fn automatic_refresh_skips_matches_outside_relevant_window() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![sample_team(
                "43922",
                "Argentina",
                "ARG",
                Confederation::Conmebol,
            )],
            matches: vec![sample_match(
                "4001",
                1,
                "Argentina",
                "France",
                "2999-06-18T12:00:00Z",
            )],
            ..AppSnapshot::default()
        });

        assert!(app.startup_refresh_resources().is_empty());
        assert!(app.background_refresh_resources().is_empty());
    }

    #[test]
    fn automatic_match_refresh_includes_related_standings_groups() {
        let mut live_match = sample_match("4001", 1, "Argentina", "France", "2999-06-18T12:00:00Z");
        live_match.group_id = Some(Group::D.id());
        live_match.group_name = Some("Group D".to_string());
        live_match.status = MatchStatus::Live;

        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            teams: vec![sample_team(
                "43922",
                "Argentina",
                "ARG",
                Confederation::Conmebol,
            )],
            matches: vec![live_match],
            ..AppSnapshot::default()
        });

        assert_eq!(
            app.background_refresh_resources(),
            vec![
                ResourceKey::Matches,
                ResourceKey::StandingsGroup(Group::D.id())
            ]
        );
    }

    #[test]
    fn future_match_details_stay_closed() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            matches: vec![sample_match(
                "4001",
                1,
                "Argentina",
                "France",
                "2999-06-18T12:00:00Z",
            )],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Matches));

        app.handle_command(AppCommand::OpenDetails);

        assert!(!app.detail_open());
        assert_eq!(app.message(), "Future matches have no details yet.");
    }

    #[test]
    fn selected_match_timeline_events_are_newest_first() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            matches: vec![sample_match(
                "4001",
                1,
                "Argentina",
                "France",
                "2026-06-18T12:00:00Z",
            )],
            timeline_events: vec![
                sample_timeline_event("4001", 1, Some(7), "0'"),
                sample_timeline_event("4001", 2, Some(0), "9'"),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Matches));

        let event_indexes = app
            .selected_match_timeline_events()
            .iter()
            .map(|event| event.event_index)
            .collect::<Vec<_>>();

        assert_eq!(event_indexes, vec![2, 1]);
    }

    #[test]
    fn match_detail_view_resets_timeline_filter_to_all() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_snapshot(AppSnapshot {
            matches: vec![
                sample_match("4001", 1, "Argentina", "France", "2026-06-18T12:00:00Z"),
                sample_match("4002", 2, "Mexico", "Canada", "2026-06-19T12:00:00Z"),
            ],
            ..AppSnapshot::default()
        });
        app.handle_command(AppCommand::Navigate(Screen::Matches));
        app.handle_command(AppCommand::OpenDetails);
        app.handle_command(AppCommand::ToggleTimelineFilter(TimelineFilter::Goals));
        assert_eq!(app.timeline_filters(), &[TimelineFilter::Goals]);

        app.handle_command(AppCommand::OpenDetails);
        assert!(app.timeline_filters().is_empty());

        app.handle_command(AppCommand::ToggleTimelineFilter(TimelineFilter::RedCards));
        assert_eq!(app.timeline_filters(), &[TimelineFilter::RedCards]);
        app.handle_command(AppCommand::FocusPrevious);
        app.handle_command(AppCommand::ScrollDown {
            max: 10,
            visible_lines: 5,
        });
        assert_eq!(app.selected_match_id(), Some(MatchId::from("4002")));
        assert!(app.timeline_filters().is_empty());
    }

    #[test]
    fn match_detail_filters_are_composable() {
        let mut app = App::new(WORLD_CUP_2026);
        app.handle_command(AppCommand::Navigate(Screen::Matches));
        app.handle_command(AppCommand::OpenDetails);

        app.handle_command(AppCommand::ToggleTimelineFilter(
            TimelineFilter::Substitutions,
        ));
        app.handle_command(AppCommand::ToggleTimelineFilter(TimelineFilter::Goals));
        assert_eq!(
            app.timeline_filters(),
            &[TimelineFilter::Goals, TimelineFilter::Substitutions]
        );
        assert_eq!(app.timeline_filter_label(), "goals, subs");

        app.handle_command(AppCommand::ToggleTimelineFilter(TimelineFilter::Goals));
        assert_eq!(app.timeline_filters(), &[TimelineFilter::Substitutions]);
    }

    #[test]
    fn last_sync_label_uses_display_time_mode() {
        let mut app = App::new(WORLD_CUP_2026);
        app.set_last_sync(Some("2000-01-01T12:00:00Z".to_string()));

        let absolute = app.last_sync_label();
        assert_ne!(absolute, "2000-01-01T12:00:00Z");
        assert!(!absolute.contains('T'));

        app.handle_command(AppCommand::ToggleTimeMode);
        assert!(app.last_sync_label().contains("ago"));
    }

    #[test]
    fn relative_time_includes_hours_when_days_are_not_exact() {
        let now = "2026-06-19T12:00:00Z".parse().expect("timestamp");
        let future = "2026-06-21T10:00:00Z".parse().expect("timestamp");
        let past = "2026-06-17T14:00:00Z".parse().expect("timestamp");

        assert_eq!(relative_time_label(future, now), "in 1d22h");
        assert_eq!(relative_time_label(past, now), "1d22h ago");
    }

    fn sample_team(
        id: &'static str,
        name: &'static str,
        abbreviation: &'static str,
        confederation: Confederation,
    ) -> Team {
        Team {
            id: TeamId::from(id),
            name: name.to_string(),
            abbreviation: abbreviation.to_string(),
            country_code: abbreviation.to_string(),
            confederation,
            flag_url_template: None,
            fifa_rank: None,
            fifa_ranking_points: None,
            favorite: false,
        }
    }

    fn sample_match(
        id: &'static str,
        match_number: u16,
        home_team_name: &'static str,
        away_team_name: &'static str,
        utc_start: &'static str,
    ) -> Match {
        Match {
            id: MatchId::from(id),
            match_number,
            stage_id: StageId::from("289273"),
            stage_name: "First Stage".to_string(),
            group_id: Some(Group::A.id()),
            group_name: Some("Group A".to_string()),
            utc_start: utc_start.parse().expect("timestamp"),
            local_start: None,
            home_team_id: None,
            away_team_id: None,
            home_team_name: home_team_name.to_string(),
            away_team_name: away_team_name.to_string(),
            home_score: None,
            away_score: None,
            home_penalty_score: None,
            away_penalty_score: None,
            status: MatchStatus::Scheduled,
            minute: None,
            stadium_name: None,
            attendance: None,
            winner_team_id: None,
        }
    }

    fn sample_timeline_event(
        match_id: &'static str,
        event_index: u16,
        event_type: Option<u16>,
        minute: &'static str,
    ) -> TimelineEvent {
        TimelineEvent {
            match_id: MatchId::from(match_id),
            event_index,
            event_type,
            team_id: None,
            player_id: None,
            minute: Some(minute.to_string()),
            description: Some("Event".to_string()),
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
            played: 1,
            won: if position == 1 { 1 } else { 0 },
            drawn: 0,
            lost: if position == 1 { 0 } else { 1 },
            goals_for: if position == 1 { 2 } else { 0 },
            goals_against: if position == 1 { 0 } else { 2 },
            goal_difference: if position == 1 { 2 } else { -2 },
            points: if position == 1 { 3 } else { 0 },
            qualification_status: None,
            fair_play: Some(-(position as i16)),
        }
    }

    fn sample_ranked_team(
        id: &'static str,
        name: &'static str,
        abbreviation: &'static str,
        fifa_rank: u16,
    ) -> Team {
        let mut team = sample_team(id, name, abbreviation, Confederation::Uefa);
        team.fifa_rank = Some(fifa_rank);
        team
    }

    fn sample_third_standing(
        group: Group,
        team_id: &'static str,
        team_name: &'static str,
        points: i16,
        goal_difference: i16,
        goals_for: i16,
        fair_play: i16,
    ) -> StandingRow {
        StandingRow {
            group_id: group.id(),
            group_name: group.name().to_string(),
            team_id: TeamId::from(team_id),
            team_name: team_name.to_string(),
            position: 3,
            played: 3,
            won: 1,
            drawn: 0,
            lost: 2,
            goals_for,
            goals_against: goals_for - goal_difference,
            goal_difference,
            points,
            qualification_status: None,
            fair_play: Some(fair_play),
        }
    }
}
