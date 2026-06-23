use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result, anyhow};
use jiff::Timestamp;
use reqwest::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::config::TournamentConfig;
use crate::data::source::DataSource;
use crate::domain::{
    Confederation, Group, GroupId, Match, MatchId, MatchStatus, PlayerStat, Stage, StandingRow,
    Team, TeamId, TimelineEvent,
};

pub const FIFA_API_BASE_URL: &str = "https://api.fifa.com/api/v3";

const LANGUAGE: &str = "en";
const DEFAULT_COUNT: &str = "500";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const NETWORK_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn check_network_access() -> Result<()> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(NETWORK_CHECK_TIMEOUT)
        .build()
        .context("failed to build FIFA network check client")?;

    client
        .get(FIFA_API_BASE_URL)
        .send()
        .await
        .with_context(|| format!("FIFA network check failed: {FIFA_API_BASE_URL}"))?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct FifaDataSource {
    client: Client,
    base_url: String,
    config: TournamentConfig,
}

impl FifaDataSource {
    pub fn new(config: TournamentConfig) -> Result<Self> {
        Self::with_base_url(FIFA_API_BASE_URL, config)
    }

    pub fn with_base_url(base_url: impl Into<String>, config: TournamentConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .context("failed to build FIFA HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.into(),
            config,
        })
    }

    async fn fetch_results<T>(&self, path: &str, query: &[(&str, String)]) -> Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let response = self
            .fetch_json::<FifaResultsResponse<T>>(path, query)
            .await?;

        Ok(response.results)
    }

    async fn fetch_json<T>(&self, path: &str, query: &[(&str, String)]) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let url = self.endpoint_url(path);
        let response = self
            .client
            .get(&url)
            .query(query)
            .send()
            .await
            .with_context(|| format!("FIFA request failed: {url}"))?
            .error_for_status()
            .with_context(|| format!("FIFA request returned an error status: {url}"))?;

        response
            .json::<T>()
            .await
            .with_context(|| format!("failed to decode FIFA response: {url}"))
    }

    fn endpoint_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

impl DataSource for FifaDataSource {
    fn teams(&self) -> impl Future<Output = Result<Vec<Team>>> + Send {
        async move {
            let path = format!("competitions/teams/{}", self.config.season_id);
            let team_query = [("language", LANGUAGE.to_owned())];
            let (teams, rankings) = tokio::try_join!(
                self.fetch_results::<FifaTeam>(&path, &team_query),
                self.rankings()
            )?;

            let rankings = rankings
                .into_iter()
                .map(|ranking| (ranking.id_team.clone(), ranking))
                .collect::<HashMap<_, _>>();

            teams
                .iter()
                .map(|team| map_team(team, rankings.get(&team.id_team)))
                .collect()
        }
    }

    fn stages(&self) -> impl Future<Output = Result<Vec<Stage>>> + Send {
        async move {
            let query = [
                ("language", LANGUAGE.to_owned()),
                ("count", "200".to_owned()),
                ("idSeason", self.config.season_id.to_owned()),
            ];
            let stages = self.fetch_results::<FifaStage>("stages", &query).await?;
            let mut stages = stages
                .iter()
                .map(|stage| {
                    Stage::from_id(&stage.id_stage)
                        .with_context(|| format!("invalid FIFA stage id {}", stage.id_stage))
                })
                .collect::<Result<Vec<_>>>()?;

            stages.sort_by_key(|stage| stage.sort_order());
            Ok(stages)
        }
    }

    fn matches(&self) -> impl Future<Output = Result<Vec<Match>>> + Send {
        async move {
            let query = [
                ("language", LANGUAGE.to_owned()),
                ("count", DEFAULT_COUNT.to_owned()),
                ("idCompetition", self.config.competition_id.to_owned()),
                ("idSeason", self.config.season_id.to_owned()),
            ];
            let matches = self
                .fetch_results::<FifaMatch>("calendar/matches", &query)
                .await?;

            matches.iter().map(map_match).collect()
        }
    }

    fn standings(
        &self,
        group: Option<GroupId>,
    ) -> impl Future<Output = Result<Vec<StandingRow>>> + Send {
        async move {
            let first_stage_id = Stage::FirstStage.id().to_string();
            let path = format!(
                "calendar/{}/{}/{}/standing",
                self.config.competition_id, self.config.season_id, first_stage_id
            );
            let mut query = vec![
                ("language", LANGUAGE.to_owned()),
                ("count", "200".to_owned()),
            ];
            if let Some(group) = group {
                query.push(("idGroup", group.into_inner()));
            }

            let standings = self.fetch_results::<FifaStandingRow>(&path, &query).await?;
            standings.iter().map(map_standing_row).collect()
        }
    }

    fn top_scorers(&self) -> impl Future<Output = Result<Vec<PlayerStat>>> + Send {
        async move {
            let path = format!(
                "topseasonplayerstatistics/season/{}/topscorers",
                self.config.season_id
            );
            let response = self
                .fetch_json::<FifaPlayerStatsResponse>(&path, &[("language", LANGUAGE.to_owned())])
                .await?;

            response
                .player_stats_list
                .iter()
                .map(map_player_stat)
                .collect()
        }
    }

    fn timeline(
        &self,
        match_id: MatchId,
    ) -> impl Future<Output = Result<Vec<TimelineEvent>>> + Send {
        async move {
            let path = format!("timelines/{match_id}");
            let response = self
                .fetch_json::<FifaTimelineResponse>(&path, &[("language", LANGUAGE.to_owned())])
                .await?;

            map_timeline_response(&response)
        }
    }
}

impl FifaDataSource {
    async fn rankings(&self) -> Result<Vec<FifaRankingRow>> {
        self.fetch_results::<FifaRankingRow>(
            "fifarankings/rankings/live",
            &[
                ("gender", "1".to_string()),
                ("sportType", "0".to_string()),
                ("language", LANGUAGE.to_owned()),
            ],
        )
        .await
    }
}

#[derive(Debug, Deserialize)]
struct FifaResultsResponse<T> {
    #[serde(rename = "Results")]
    results: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct FifaLocalizedText {
    #[serde(rename = "Locale")]
    locale: Option<String>,
    #[serde(rename = "Description")]
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FifaTeam {
    #[serde(rename = "IdTeam")]
    id_team: String,
    #[serde(rename = "IdConfederation")]
    id_confederation: String,
    #[serde(rename = "Name", default)]
    name: Vec<FifaLocalizedText>,
    #[serde(rename = "IdCountry")]
    id_country: String,
    #[serde(rename = "ShortClubName")]
    short_club_name: Option<String>,
    #[serde(rename = "Abbreviation")]
    abbreviation: Option<String>,
    #[serde(rename = "PictureUrl")]
    picture_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FifaRankingRow {
    #[serde(rename = "IdTeam")]
    id_team: String,
    #[serde(rename = "Rank")]
    rank: Option<i64>,
    #[serde(rename = "TotalPoints")]
    total_points: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct FifaStage {
    #[serde(rename = "IdStage")]
    id_stage: String,
}

#[derive(Debug, Deserialize)]
struct FifaMatch {
    #[serde(rename = "IdStage")]
    id_stage: String,
    #[serde(rename = "IdGroup")]
    id_group: Option<String>,
    #[serde(rename = "IdMatch")]
    id_match: String,
    #[serde(rename = "MatchNumber")]
    match_number: Option<i64>,
    #[serde(rename = "StageName", default)]
    stage_name: Vec<FifaLocalizedText>,
    #[serde(rename = "GroupName", default)]
    group_name: Vec<FifaLocalizedText>,
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Home")]
    home: Option<FifaMatchTeam>,
    #[serde(rename = "Away")]
    away: Option<FifaMatchTeam>,
    #[serde(rename = "HomeTeamScore")]
    home_team_score: Option<i64>,
    #[serde(rename = "AwayTeamScore")]
    away_team_score: Option<i64>,
    #[serde(rename = "HomeTeamPenaltyScore")]
    home_team_penalty_score: Option<i64>,
    #[serde(rename = "AwayTeamPenaltyScore")]
    away_team_penalty_score: Option<i64>,
    #[serde(rename = "MatchStatus")]
    match_status: Option<i64>,
    #[serde(rename = "ResultType")]
    result_type: Option<i64>,
    #[serde(rename = "MatchTime")]
    match_time: Option<String>,
    #[serde(rename = "Stadium")]
    stadium: Option<FifaStadium>,
    #[serde(rename = "Attendance")]
    attendance: Option<String>,
    #[serde(rename = "Winner")]
    winner: Option<String>,
    #[serde(rename = "PlaceHolderA")]
    placeholder_a: Option<String>,
    #[serde(rename = "PlaceHolderB")]
    placeholder_b: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FifaMatchTeam {
    #[serde(rename = "Score")]
    score: Option<i64>,
    #[serde(rename = "IdTeam")]
    id_team: Option<String>,
    #[serde(rename = "TeamName", default)]
    team_name: Vec<FifaLocalizedText>,
    #[serde(rename = "ShortClubName")]
    short_club_name: Option<String>,
    #[serde(rename = "Abbreviation")]
    abbreviation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FifaStadium {
    #[serde(rename = "Name", default)]
    name: Vec<FifaLocalizedText>,
}

#[derive(Debug, Deserialize)]
struct FifaStandingRow {
    #[serde(rename = "IdGroup")]
    id_group: String,
    #[serde(rename = "IdTeam")]
    id_team: String,
    #[serde(rename = "Group", default)]
    group: Vec<FifaLocalizedText>,
    #[serde(rename = "Team")]
    team: Option<FifaTeam>,
    #[serde(rename = "Position")]
    position: Option<i64>,
    #[serde(rename = "Played")]
    played: Option<i64>,
    #[serde(rename = "Won")]
    won: Option<i64>,
    #[serde(rename = "Drawn")]
    drawn: Option<i64>,
    #[serde(rename = "Lost")]
    lost: Option<i64>,
    #[serde(rename = "For")]
    goals_for: Option<i64>,
    #[serde(rename = "Against")]
    goals_against: Option<i64>,
    #[serde(rename = "GoalsDiference")]
    goal_difference: Option<i64>,
    #[serde(rename = "Points")]
    points: Option<i64>,
    #[serde(rename = "QualificationStatus")]
    qualification_status: Option<String>,
    #[serde(rename = "TeamConductScore")]
    team_conduct_score: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct FifaPlayerStatsResponse {
    #[serde(rename = "PlayerStatsList", default)]
    player_stats_list: Vec<FifaPlayerStat>,
}

#[derive(Debug, Deserialize)]
struct FifaPlayerStat {
    #[serde(rename = "PlayerInfo")]
    player_info: FifaPlayerInfo,
    #[serde(rename = "Rank")]
    rank: Option<i64>,
    #[serde(rename = "MatchesPlayed")]
    matches_played: Option<i64>,
    #[serde(rename = "ActualMinutesPlayed")]
    actual_minutes_played: Option<i64>,
    #[serde(rename = "GoalsScored")]
    goals_scored: Option<i64>,
    #[serde(rename = "Assists")]
    assists: Option<i64>,
    #[serde(rename = "TotalAttempts")]
    total_attempts: Option<i64>,
    #[serde(rename = "AttemptsOnTarget")]
    attempts_on_target: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct FifaPlayerInfo {
    #[serde(rename = "IdPlayer")]
    id_player: String,
    #[serde(rename = "IdCountry")]
    id_country: String,
    #[serde(rename = "PlayerName", default)]
    player_name: Vec<FifaLocalizedText>,
    #[serde(rename = "IdTeam")]
    id_team: String,
}

#[derive(Debug, Deserialize)]
struct FifaTimelineResponse {
    #[serde(rename = "IdMatch")]
    id_match: String,
    #[serde(rename = "Event", default)]
    event: Vec<FifaTimelineEvent>,
}

#[derive(Debug, Deserialize)]
struct FifaTimelineEvent {
    #[serde(rename = "IdTeam")]
    id_team: Option<String>,
    #[serde(rename = "IdPlayer")]
    id_player: Option<String>,
    #[serde(rename = "MatchMinute")]
    match_minute: Option<String>,
    #[serde(rename = "Type")]
    event_type: Option<i64>,
    #[serde(rename = "EventDescription", default)]
    event_description: Vec<FifaLocalizedText>,
}

fn map_team(team: &FifaTeam, ranking: Option<&FifaRankingRow>) -> Result<Team> {
    Ok(Team {
        id: TeamId::from(required_non_empty(&team.id_team, "IdTeam")?),
        name: fifa_team_name(team).context("FIFA team missing display name")?,
        abbreviation: required_owned(team.abbreviation.as_deref(), "Abbreviation")?,
        country_code: required_owned(Some(&team.id_country), "IdCountry")?,
        confederation: Confederation::from_code(required_non_empty(
            &team.id_confederation,
            "IdConfederation",
        )?)?,
        flag_url_template: optional_owned(team.picture_url.as_deref()),
        fifa_rank: ranking
            .and_then(|ranking| ranking.rank)
            .map(|rank| required_u16(Some(rank), "Rank"))
            .transpose()?,
        fifa_ranking_points: ranking.and_then(|ranking| ranking.total_points),
        favorite: false,
    })
}

fn map_match(match_: &FifaMatch) -> Result<Match> {
    let stage = Stage::from_id(required_non_empty(&match_.id_stage, "IdStage")?)?;
    let group = match match_.id_group.as_deref().and_then(non_empty) {
        Some(value) => Some(Group::from_id(value)?),
        None => None,
    };
    let home_score = optional_u8(
        match_
            .home_team_score
            .or_else(|| match_.home.as_ref().and_then(|team| team.score)),
        "HomeTeamScore",
    )?;
    let away_score = optional_u8(
        match_
            .away_team_score
            .or_else(|| match_.away.as_ref().and_then(|team| team.score)),
        "AwayTeamScore",
    )?;

    Ok(Match {
        id: MatchId::from(required_non_empty(&match_.id_match, "IdMatch")?),
        match_number: required_u16(match_.match_number, "MatchNumber")?,
        stage_id: stage.id(),
        stage_name: localized_text(&match_.stage_name).unwrap_or_else(|| stage.name().to_owned()),
        group_id: group.map(|group| group.id()),
        group_name: group
            .map(|group| localized_text(&match_.group_name).unwrap_or_else(|| group.name().into())),
        utc_start: parse_timestamp(&match_.date, "Date")?,
        local_start: None,
        home_team_id: optional_team_id(
            match_
                .home
                .as_ref()
                .and_then(|team| team.id_team.as_deref()),
        ),
        away_team_id: optional_team_id(
            match_
                .away
                .as_ref()
                .and_then(|team| team.id_team.as_deref()),
        ),
        home_team_name: fifa_match_team_name(
            match_.home.as_ref(),
            match_.placeholder_a.as_deref(),
            "Home",
        ),
        away_team_name: fifa_match_team_name(
            match_.away.as_ref(),
            match_.placeholder_b.as_deref(),
            "Away",
        ),
        home_score,
        away_score,
        home_penalty_score: optional_u8(match_.home_team_penalty_score, "HomeTeamPenaltyScore")?,
        away_penalty_score: optional_u8(match_.away_team_penalty_score, "AwayTeamPenaltyScore")?,
        status: map_match_status(
            match_.match_status,
            match_.result_type,
            home_score.is_some() || away_score.is_some(),
        ),
        minute: optional_owned(match_.match_time.as_deref()),
        stadium_name: match_
            .stadium
            .as_ref()
            .and_then(|stadium| localized_text(&stadium.name)),
        attendance: parse_attendance(match_.attendance.as_deref())?,
        winner_team_id: optional_team_id(match_.winner.as_deref()),
    })
}

fn map_standing_row(row: &FifaStandingRow) -> Result<StandingRow> {
    let group = Group::from_id(required_non_empty(&row.id_group, "IdGroup")?)?;
    let team_name = row
        .team
        .as_ref()
        .and_then(fifa_team_name)
        .context("FIFA standing row missing team display name")?;

    Ok(StandingRow {
        group_id: group.id(),
        group_name: localized_text(&row.group).unwrap_or_else(|| group.name().to_owned()),
        team_id: TeamId::from(required_non_empty(&row.id_team, "IdTeam")?),
        team_name,
        position: required_u8(row.position, "Position")?,
        played: required_u8(row.played, "Played")?,
        won: required_u8(row.won, "Won")?,
        drawn: required_u8(row.drawn, "Drawn")?,
        lost: required_u8(row.lost, "Lost")?,
        goals_for: required_i16(row.goals_for, "For")?,
        goals_against: required_i16(row.goals_against, "Against")?,
        goal_difference: required_i16(row.goal_difference, "GoalsDiference")?,
        points: required_i16(row.points, "Points")?,
        qualification_status: optional_owned(row.qualification_status.as_deref()),
        fair_play: optional_i16(row.team_conduct_score, "TeamConductScore")?,
    })
}

fn map_player_stat(stat: &FifaPlayerStat) -> Result<PlayerStat> {
    Ok(PlayerStat {
        player_id: required_non_empty(&stat.player_info.id_player, "IdPlayer")?.to_owned(),
        player_name: localized_text(&stat.player_info.player_name)
            .context("FIFA player stat missing player name")?,
        team_id: TeamId::from(required_non_empty(&stat.player_info.id_team, "IdTeam")?),
        country_code: required_non_empty(&stat.player_info.id_country, "IdCountry")?.to_owned(),
        rank: required_u16(stat.rank, "Rank")?,
        matches_played: required_u16(stat.matches_played, "MatchesPlayed")?,
        minutes_played: required_u16(stat.actual_minutes_played, "ActualMinutesPlayed")?,
        goals: optional_u16(stat.goals_scored, "GoalsScored")?.unwrap_or_default(),
        assists: optional_u16(stat.assists, "Assists")?.unwrap_or_default(),
        attempts: optional_u16(stat.total_attempts, "TotalAttempts")?.unwrap_or_default(),
        attempts_on_target: optional_u16(stat.attempts_on_target, "AttemptsOnTarget")?
            .unwrap_or_default(),
    })
}

fn map_timeline_response(response: &FifaTimelineResponse) -> Result<Vec<TimelineEvent>> {
    let match_id = MatchId::from(required_non_empty(&response.id_match, "IdMatch")?);

    response
        .event
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let event_index = u16::try_from(index + 1)
                .map_err(|_| anyhow!("timeline event index exceeds u16: {}", index + 1))?;

            Ok(TimelineEvent {
                match_id: match_id.clone(),
                event_index,
                event_type: optional_u16(event.event_type, "Type")?,
                team_id: optional_team_id(event.id_team.as_deref()),
                player_id: optional_owned(event.id_player.as_deref()),
                minute: optional_owned(event.match_minute.as_deref()),
                description: localized_text(&event.event_description),
            })
        })
        .collect()
}

fn fifa_team_name(team: &FifaTeam) -> Option<String> {
    localized_text(&team.name)
        .or_else(|| optional_owned(team.short_club_name.as_deref()))
        .or_else(|| optional_owned(team.abbreviation.as_deref()))
}

fn fifa_match_team_name(
    team: Option<&FifaMatchTeam>,
    placeholder: Option<&str>,
    fallback: &str,
) -> String {
    team.and_then(|team| {
        localized_text(&team.team_name)
            .or_else(|| optional_owned(team.short_club_name.as_deref()))
            .or_else(|| optional_owned(team.abbreviation.as_deref()))
    })
    .or_else(|| optional_owned(placeholder))
    .unwrap_or_else(|| fallback.to_owned())
}

fn localized_text(values: &[FifaLocalizedText]) -> Option<String> {
    values
        .iter()
        .find(|value| value.locale.as_deref() == Some("en-GB"))
        .and_then(|value| value.description.as_deref())
        .and_then(non_empty)
        .or_else(|| {
            values
                .iter()
                .find_map(|value| value.description.as_deref().and_then(non_empty))
        })
        .map(str::to_owned)
}

fn map_match_status(
    raw_status: Option<i64>,
    result_type: Option<i64>,
    has_score: bool,
) -> MatchStatus {
    match raw_status {
        Some(1) => MatchStatus::Scheduled,
        Some(3) => MatchStatus::Live,
        Some(0) if has_score || result_type.unwrap_or_default() > 0 => MatchStatus::FullTime,
        Some(0) => MatchStatus::Unknown(0),
        Some(value) if (0..=u16::MAX as i64).contains(&value) => MatchStatus::Unknown(value as u16),
        Some(_) | None if has_score => MatchStatus::FullTime,
        Some(_) | None => MatchStatus::Unknown(0),
    }
}

fn parse_timestamp(value: &str, field: &str) -> Result<Timestamp> {
    required_non_empty(value, field)?
        .parse::<Timestamp>()
        .with_context(|| format!("invalid FIFA timestamp in {field}: {value}"))
}

fn parse_attendance(value: Option<&str>) -> Result<Option<u32>> {
    let Some(value) = value.and_then(non_empty) else {
        return Ok(None);
    };
    let normalized = value.replace(',', "");

    normalized
        .parse::<u32>()
        .map(Some)
        .with_context(|| format!("invalid FIFA attendance value: {value}"))
}

fn optional_team_id(value: Option<&str>) -> Option<TeamId> {
    match value.and_then(non_empty) {
        Some("0") | None => None,
        Some(value) => Some(TeamId::from(value)),
    }
}

fn optional_owned(value: Option<&str>) -> Option<String> {
    value.and_then(non_empty).map(str::to_owned)
}

fn required_owned(value: Option<&str>, field: &str) -> Result<String> {
    value
        .and_then(non_empty)
        .map(str::to_owned)
        .with_context(|| format!("FIFA payload missing required field {field}"))
}

fn required_non_empty<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    non_empty(value).with_context(|| format!("FIFA payload missing required field {field}"))
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn required_u8(value: Option<i64>, field: &str) -> Result<u8> {
    optional_u8(value, field)?
        .with_context(|| format!("FIFA payload missing required field {field}"))
}

fn optional_u8(value: Option<i64>, field: &str) -> Result<Option<u8>> {
    let Some(value) = value else {
        return Ok(None);
    };

    u8::try_from(value)
        .map(Some)
        .with_context(|| format!("FIFA field {field} is outside u8 range: {value}"))
}

fn required_u16(value: Option<i64>, field: &str) -> Result<u16> {
    optional_u16(value, field)?
        .with_context(|| format!("FIFA payload missing required field {field}"))
}

fn optional_u16(value: Option<i64>, field: &str) -> Result<Option<u16>> {
    let Some(value) = value else {
        return Ok(None);
    };

    u16::try_from(value)
        .map(Some)
        .with_context(|| format!("FIFA field {field} is outside u16 range: {value}"))
}

fn required_i16(value: Option<i64>, field: &str) -> Result<i16> {
    optional_i16(value, field)?
        .with_context(|| format!("FIFA payload missing required field {field}"))
}

fn optional_i16(value: Option<i64>, field: &str) -> Result<Option<i16>> {
    let Some(value) = value else {
        return Ok(None);
    };

    i16::try_from(value)
        .map(Some)
        .with_context(|| format!("FIFA field {field} is outside i16 range: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_team_payload() {
        let team: FifaTeam = serde_json::from_value(json!({
            "IdTeam": "43922",
            "IdConfederation": "CONMEBOL",
            "Name": [{"Locale": "en-GB", "Description": "Argentina"}],
            "IdCountry": "ARG",
            "ShortClubName": "Argentina",
            "Abbreviation": "ARG",
            "PictureUrl": "https://api.fifa.com/api/v3/picture/flags-{format}-{size}/ARG"
        }))
        .unwrap();

        let ranking = FifaRankingRow {
            id_team: "43922".to_string(),
            rank: Some(1),
            total_points: Some(1886.16),
        };
        let team = map_team(&team, Some(&ranking)).unwrap();

        assert_eq!(team.id, TeamId::from("43922"));
        assert_eq!(team.name, "Argentina");
        assert_eq!(team.abbreviation, "ARG");
        assert_eq!(team.country_code, "ARG");
        assert_eq!(team.confederation, Confederation::Conmebol);
        assert_eq!(team.fifa_rank, Some(1));
        assert_eq!(team.fifa_ranking_points, Some(1886.16));
        assert!(!team.favorite);
    }

    #[test]
    fn maps_match_payload() {
        let match_: FifaMatch = serde_json::from_value(json!({
            "IdStage": "289273",
            "IdGroup": "289275",
            "IdMatch": "400021443",
            "MatchNumber": 1,
            "StageName": [{"Locale": "en-GB", "Description": "First Stage"}],
            "GroupName": [{"Locale": "en-GB", "Description": "Group A"}],
            "Date": "2026-06-11T19:00:00Z",
            "Home": {
                "Score": 2,
                "IdTeam": "43911",
                "TeamName": [{"Locale": "en-GB", "Description": "Mexico"}],
                "ShortClubName": "Mexico",
                "Abbreviation": "MEX"
            },
            "Away": {
                "Score": 0,
                "IdTeam": "43883",
                "TeamName": [{"Locale": "en-GB", "Description": "South Africa"}],
                "ShortClubName": "South Africa",
                "Abbreviation": "RSA"
            },
            "HomeTeamScore": 2,
            "AwayTeamScore": 0,
            "HomeTeamPenaltyScore": null,
            "AwayTeamPenaltyScore": null,
            "MatchStatus": 0,
            "ResultType": 1,
            "MatchTime": "98'",
            "Stadium": {
                "Name": [{"Locale": "en-GB", "Description": "Mexico City Stadium"}]
            },
            "Attendance": "80824",
            "Winner": "43911"
        }))
        .unwrap();

        let match_ = map_match(&match_).unwrap();

        assert_eq!(match_.id, MatchId::from("400021443"));
        assert_eq!(match_.match_number, 1);
        assert_eq!(match_.stage_id, Stage::FirstStage.id());
        assert_eq!(match_.group_id, Some(Group::A.id()));
        assert_eq!(match_.home_team_name, "Mexico");
        assert_eq!(match_.away_team_name, "South Africa");
        assert_eq!(match_.home_score, Some(2));
        assert_eq!(match_.away_score, Some(0));
        assert_eq!(match_.status, MatchStatus::FullTime);
        assert_eq!(match_.minute.as_deref(), Some("98'"));
        assert_eq!(match_.stadium_name.as_deref(), Some("Mexico City Stadium"));
        assert_eq!(match_.attendance, Some(80824));
        assert_eq!(match_.winner_team_id, Some(TeamId::from("43911")));
    }

    #[test]
    fn maps_future_match_placeholders_without_team_ids() {
        let match_: FifaMatch = serde_json::from_value(json!({
            "IdStage": "289287",
            "IdMatch": "400021500",
            "MatchNumber": 73,
            "StageName": [{"Locale": "en-GB", "Description": "Round of 32"}],
            "Date": "2026-06-28T19:00:00Z",
            "Home": null,
            "Away": null,
            "HomeTeamScore": null,
            "AwayTeamScore": null,
            "MatchStatus": 1,
            "ResultType": 0,
            "PlaceHolderA": "Winner Group A",
            "PlaceHolderB": "Runner-up Group B"
        }))
        .unwrap();

        let match_ = map_match(&match_).unwrap();

        assert_eq!(match_.status, MatchStatus::Scheduled);
        assert_eq!(match_.home_team_id, None);
        assert_eq!(match_.away_team_id, None);
        assert_eq!(match_.home_team_name, "Winner Group A");
        assert_eq!(match_.away_team_name, "Runner-up Group B");
    }

    #[test]
    fn maps_standing_payload() {
        let row: FifaStandingRow = serde_json::from_value(json!({
            "IdGroup": "289275",
            "IdTeam": "43911",
            "Group": [{"Locale": "en-GB", "Description": "Group A"}],
            "Team": {
                "IdTeam": "43911",
                "IdConfederation": "CONCACAF",
                "Name": [{"Locale": "en-GB", "Description": "Mexico"}],
                "IdCountry": "MEX",
                "ShortClubName": "Mexico",
                "Abbreviation": "MEX",
                "PictureUrl": null
            },
            "Position": 2,
            "Played": 1,
            "Won": 1,
            "Drawn": 0,
            "Lost": 0,
            "For": 2,
            "Against": 0,
            "GoalsDiference": 2,
            "Points": 3,
            "QualificationStatus": "CouldQualify",
            "TeamConductScore": -5
        }))
        .unwrap();

        let row = map_standing_row(&row).unwrap();

        assert_eq!(row.group_id, Group::A.id());
        assert_eq!(row.team_id, TeamId::from("43911"));
        assert_eq!(row.team_name, "Mexico");
        assert_eq!(row.position, 2);
        assert_eq!(row.points, 3);
        assert_eq!(row.goal_difference, 2);
        assert_eq!(row.fair_play, Some(-5));
    }

    #[test]
    fn maps_player_stat_payload() {
        let stat: FifaPlayerStat = serde_json::from_value(json!({
            "PlayerInfo": {
                "IdPlayer": "229397",
                "IdCountry": "ARG",
                "PlayerName": [{"Locale": "en-GB", "Description": "Lionel MESSI"}],
                "IdTeam": "43922"
            },
            "Rank": 1,
            "MatchesPlayed": 1,
            "ActualMinutesPlayed": 83,
            "GoalsScored": 3,
            "Assists": 0,
            "TotalAttempts": 6,
            "AttemptsOnTarget": 4
        }))
        .unwrap();

        let stat = map_player_stat(&stat).unwrap();

        assert_eq!(stat.player_id, "229397");
        assert_eq!(stat.player_name, "Lionel MESSI");
        assert_eq!(stat.team_id, TeamId::from("43922"));
        assert_eq!(stat.country_code, "ARG");
        assert_eq!(stat.rank, 1);
        assert_eq!(stat.goals, 3);
        assert_eq!(stat.assists, 0);
        assert_eq!(stat.attempts_on_target, 4);
    }

    #[test]
    fn maps_timeline_payload() {
        let timeline: FifaTimelineResponse = serde_json::from_value(json!({
            "IdMatch": "400021443",
            "Event": [
                {
                    "IdTeam": "43883",
                    "IdPlayer": "395050",
                    "MatchMinute": "3'",
                    "Type": 18,
                    "EventDescription": [
                        {"Locale": "en-GB", "Description": "MODIBA (South Africa) commits a foul."}
                    ]
                }
            ]
        }))
        .unwrap();

        let events = map_timeline_response(&timeline).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].match_id, MatchId::from("400021443"));
        assert_eq!(events[0].event_index, 1);
        assert_eq!(events[0].event_type, Some(18));
        assert_eq!(events[0].team_id, Some(TeamId::from("43883")));
        assert_eq!(events[0].player_id.as_deref(), Some("395050"));
        assert_eq!(events[0].minute.as_deref(), Some("3'"));
        assert_eq!(
            events[0].description.as_deref(),
            Some("MODIBA (South Africa) commits a foul.")
        );
    }

    #[test]
    fn rejects_unknown_fixed_tournament_ids() {
        let match_: FifaMatch = serde_json::from_value(json!({
            "IdStage": "999",
            "IdMatch": "400021443",
            "MatchNumber": 1,
            "Date": "2026-06-11T19:00:00Z"
        }))
        .unwrap();

        assert!(map_match(&match_).is_err());
    }
}
