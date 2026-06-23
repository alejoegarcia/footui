use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use directories::ProjectDirs;
use jiff::{Timestamp, Zoned};
use rusqlite::{Connection, params};

use crate::{
    config::WORLD_CUP_2026,
    data::repository::{AppSnapshot, Repository, SyncResult},
    domain::{
        Group, GroupId, Match, MatchId, MatchStatus, Stage, StandingRow, Team, TeamId,
        TimelineEvent,
    },
};

const MIN_SQLITE_VERSION: i32 = 3_045_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DbLocation {
    ProjectLocal,
    AppData,
}

impl DbLocation {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ProjectLocal => "project-local",
            Self::AppData => "app-data",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseInfo {
    pub path: PathBuf,
    pub location: DbLocation,
    pub sqlite_version: String,
    pub applied_migrations: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Migration {
    version: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "202606180001_init",
        sql: include_str!("migrations/202606180001_init.sql"),
    },
    Migration {
        version: "202606180002_sync_metadata",
        sql: include_str!("migrations/202606180002_sync_metadata.sql"),
    },
    Migration {
        version: "202606180003_standings_fair_play",
        sql: include_str!("migrations/202606180003_standings_fair_play.sql"),
    },
    Migration {
        version: "202606180004_team_rankings",
        sql: include_str!("migrations/202606180004_team_rankings.sql"),
    },
];

pub fn initialize() -> Result<DatabaseInfo> {
    assert_sqlite_version()?;

    let (path, location) = database_path()?;
    initialize_at(path, location)
}

pub fn initialize_at(path: PathBuf, location: DbLocation) -> Result<DatabaseInfo> {
    assert_sqlite_version()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating database directory {}", parent.display()))?;
    }

    let mut connection =
        Connection::open(&path).with_context(|| format!("opening database {}", path.display()))?;
    configure_connection(&connection)?;
    let applied_migrations = apply_migrations(&mut connection)?;

    Ok(DatabaseInfo {
        path,
        location,
        sqlite_version: rusqlite::version().to_string(),
        applied_migrations,
    })
}

pub fn latest_data_updated_at(path: &Path) -> Result<Option<String>> {
    let connection =
        Connection::open(path).with_context(|| format!("opening database {}", path.display()))?;
    configure_connection(&connection)?;

    connection
        .query_row(
            "
            select max(value)
            from (
              select last_success_at as value
              from resource_refreshes
              where last_success_at is not null
              union all
              select updated_at as value
              from teams
              union all
              select updated_at as value
              from matches
              union all
              select updated_at as value
              from standings
            )
            ",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .context("loading latest data update timestamp")
}

fn assert_sqlite_version() -> Result<()> {
    let version_number = rusqlite::version_number();
    if version_number < MIN_SQLITE_VERSION {
        bail!(
            "SQLite {} is too old; footui requires SQLite >= 3.45.0 for JSONB support",
            rusqlite::version()
        );
    }

    Ok(())
}

fn database_path() -> Result<(PathBuf, DbLocation)> {
    if cfg!(debug_assertions) {
        return Ok((
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(".footui")
                .join("footui.sqlite3"),
            DbLocation::ProjectLocal,
        ));
    }

    let project_dirs = ProjectDirs::from("dev", "footui", "footui")
        .ok_or_else(|| anyhow!("could not resolve OS app-data directory"))?;
    Ok((
        project_dirs.data_dir().join("footui.sqlite3"),
        DbLocation::AppData,
    ))
}

pub(crate) fn configure_connection(connection: &Connection) -> Result<()> {
    connection
        .execute_batch(
            "
            pragma foreign_keys = on;
            pragma journal_mode = wal;
            pragma synchronous = normal;
            pragma busy_timeout = 5000;
            ",
        )
        .context("configuring SQLite connection")?;

    Ok(())
}

#[derive(Clone, Debug)]
pub struct SqliteRepository {
    path: PathBuf,
}

impl SqliteRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn open(&self) -> Result<Connection> {
        let connection = Connection::open(&self.path)
            .with_context(|| format!("opening database {}", self.path.display()))?;
        configure_connection(&connection)?;
        Ok(connection)
    }
}

impl Repository for SqliteRepository {
    fn load_snapshot(&self) -> Result<AppSnapshot> {
        let connection = self.open()?;

        Ok(AppSnapshot {
            teams: load_teams(&connection)?,
            matches: load_matches(&connection)?,
            standings: load_standings(&connection)?,
            top_scorers: Vec::new(),
            timeline_events: load_timeline_events(&connection)?,
        })
    }

    fn save_sync_result(&self, result: SyncResult) -> Result<()> {
        let mut connection = self.open()?;
        let transaction = connection
            .transaction()
            .context("starting sync transaction")?;

        seed_static_tournament(&transaction)?;
        save_teams(&transaction, &result.teams)?;
        save_matches(&transaction, &result.matches)?;
        save_standings(&transaction, &result.standings)?;
        save_timeline_events(&transaction, &result.timeline_events)?;

        transaction
            .commit()
            .context("committing sync transaction")?;
        Ok(())
    }

    fn toggle_favorite_team(&self, team_id: TeamId) -> Result<()> {
        let connection = self.open()?;
        let changed = connection
            .execute(
                "delete from favorite_teams where team_id = ?1",
                params![team_id.as_str()],
            )
            .with_context(|| format!("removing favorite team {team_id}"))?;

        if changed == 0 {
            connection
                .execute(
                    "
                    insert into favorite_teams(team_id, created_at)
                    values(?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    ",
                    params![team_id.as_str()],
                )
                .with_context(|| format!("adding favorite team {team_id}"))?;
        }

        Ok(())
    }
}

fn load_teams(connection: &Connection) -> Result<Vec<Team>> {
    let mut statement = connection
        .prepare(
            "
            select
              teams.id,
              teams.name,
              teams.abbreviation,
              teams.country_code,
              teams.confederation,
              teams.flag_url_template,
              teams.fifa_rank,
              teams.fifa_ranking_points,
              favorite_teams.team_id is not null as favorite
            from teams
            left join favorite_teams on favorite_teams.team_id = teams.id
            order by teams.name collate nocase asc
            ",
        )
        .context("preparing team snapshot query")?;

    let teams = statement
        .query_map([], |row| {
            let confederation: String = row.get(4)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                confederation,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<u16>>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, bool>(8)?,
            ))
        })
        .context("querying team snapshot")?
        .map(|row| {
            let (
                id,
                name,
                abbreviation,
                country_code,
                confederation,
                flag_url_template,
                fifa_rank,
                fifa_ranking_points,
                favorite,
            ) = row.context("reading team row")?;
            Ok(Team {
                id: TeamId::from(id),
                name,
                abbreviation,
                country_code,
                confederation: crate::domain::Confederation::from_code(&confederation)?,
                flag_url_template,
                fifa_rank,
                fifa_ranking_points,
                favorite,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(teams)
}

fn load_matches(connection: &Connection) -> Result<Vec<Match>> {
    let mut statement = connection
        .prepare(
            "
            select
              matches.id,
              matches.match_number,
              matches.stage_id,
              stages.name as stage_name,
              matches.group_id,
              groups.name as group_name,
              matches.utc_start,
              matches.local_start,
              matches.home_team_id,
              matches.away_team_id,
              matches.home_team_name,
              matches.away_team_name,
              matches.home_score,
              matches.away_score,
              matches.home_penalty_score,
              matches.away_penalty_score,
              matches.status,
              matches.minute,
              matches.stadium_name,
              matches.attendance,
              matches.winner_team_id
            from matches
            join stages on stages.id = matches.stage_id
            left join groups on groups.id = matches.group_id
            order by matches.utc_start asc, matches.match_number asc
            ",
        )
        .context("preparing match snapshot query")?;

    let matches = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u16>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, Option<u8>>(12)?,
                row.get::<_, Option<u8>>(13)?,
                row.get::<_, Option<u8>>(14)?,
                row.get::<_, Option<u8>>(15)?,
                row.get::<_, u16>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, Option<String>>(18)?,
                row.get::<_, Option<u32>>(19)?,
                row.get::<_, Option<String>>(20)?,
            ))
        })
        .context("querying match snapshot")?
        .map(|row| {
            let (
                id,
                match_number,
                stage_id,
                stage_name,
                group_id,
                group_name,
                utc_start,
                local_start,
                home_team_id,
                away_team_id,
                home_team_name,
                away_team_name,
                home_score,
                away_score,
                home_penalty_score,
                away_penalty_score,
                status,
                minute,
                stadium_name,
                attendance,
                winner_team_id,
            ) = row.context("reading match row")?;

            Ok(Match {
                id: MatchId::from(id),
                match_number,
                stage_id: Stage::from_id(&stage_id)?.id(),
                stage_name,
                group_id: group_id.map(GroupId::from),
                group_name,
                utc_start: parse_timestamp(&utc_start, "matches.utc_start")?,
                local_start: parse_optional_zoned(local_start.as_deref())?,
                home_team_id: home_team_id.map(TeamId::from),
                away_team_id: away_team_id.map(TeamId::from),
                home_team_name,
                away_team_name,
                home_score,
                away_score,
                home_penalty_score,
                away_penalty_score,
                status: match_status_from_storage(status),
                minute,
                stadium_name,
                attendance,
                winner_team_id: winner_team_id.map(TeamId::from),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(matches)
}

fn load_timeline_events(connection: &Connection) -> Result<Vec<TimelineEvent>> {
    let mut statement = connection
        .prepare(
            "
            select
              match_id,
              event_index,
              event_type,
              team_id,
              player_id,
              minute,
              description
            from timeline_events
            order by match_id asc, event_index asc
            ",
        )
        .context("preparing timeline snapshot query")?;

    let events = statement
        .query_map([], |row| {
            Ok(TimelineEvent {
                match_id: MatchId::from(row.get::<_, String>(0)?),
                event_index: row.get(1)?,
                event_type: row.get(2)?,
                team_id: row.get::<_, Option<String>>(3)?.map(TeamId::from),
                player_id: row.get(4)?,
                minute: row.get(5)?,
                description: row.get(6)?,
            })
        })
        .context("querying timeline snapshot")?
        .map(|row| row.context("reading timeline event row"))
        .collect::<Result<Vec<_>>>()?;

    Ok(events)
}

fn load_standings(connection: &Connection) -> Result<Vec<StandingRow>> {
    let mut statement = connection
        .prepare(
            "
            select
              standings.group_id,
              groups.name as group_name,
              standings.team_id,
              teams.name as team_name,
              standings.position,
              standings.played,
              standings.won,
              standings.drawn,
              standings.lost,
              standings.goals_for,
              standings.goals_against,
              standings.goal_difference,
              standings.points,
              standings.qualification_status,
              standings.fair_play
            from standings
            join groups on groups.id = standings.group_id
            join teams on teams.id = standings.team_id
            where standings.season_id = ?1
            order by standings.group_id asc, standings.position asc
            ",
        )
        .context("preparing standings snapshot query")?;

    let standings = statement
        .query_map([WORLD_CUP_2026.season_id], |row| {
            Ok(StandingRow {
                group_id: GroupId::from(row.get::<_, String>(0)?),
                group_name: row.get(1)?,
                team_id: TeamId::from(row.get::<_, String>(2)?),
                team_name: row.get(3)?,
                position: row.get(4)?,
                played: row.get(5)?,
                won: row.get(6)?,
                drawn: row.get(7)?,
                lost: row.get(8)?,
                goals_for: row.get(9)?,
                goals_against: row.get(10)?,
                goal_difference: row.get(11)?,
                points: row.get(12)?,
                qualification_status: row.get(13)?,
                fair_play: row.get(14)?,
            })
        })
        .context("querying standings snapshot")?
        .map(|row| row.context("reading standings row"))
        .collect::<Result<Vec<_>>>()?;

    Ok(standings)
}

fn seed_static_tournament(connection: &Connection) -> Result<()> {
    for stage in Stage::ALL {
        connection
            .execute(
                "
                insert into stages(id, name, stage_type, sort_order)
                values(?1, ?2, ?3, ?4)
                on conflict(id) do update set
                  name = excluded.name,
                  stage_type = excluded.stage_type,
                  sort_order = excluded.sort_order
                ",
                params![
                    stage.id().to_string(),
                    stage.name(),
                    if stage == Stage::FirstStage { 1 } else { 0 },
                    stage.sort_order()
                ],
            )
            .with_context(|| format!("seeding stage {}", stage.name()))?;
    }

    for group in Group::ALL {
        connection
            .execute(
                "
                insert into groups(id, name, stage_id)
                values(?1, ?2, ?3)
                on conflict(id) do update set
                  name = excluded.name,
                  stage_id = excluded.stage_id
                ",
                params![
                    group.id().to_string(),
                    group.name(),
                    Stage::FirstStage.id().to_string()
                ],
            )
            .with_context(|| format!("seeding group {}", group.name()))?;
    }

    Ok(())
}

fn save_teams(connection: &Connection, teams: &[Team]) -> Result<()> {
    for team in teams {
        connection
            .execute(
                "
                insert into teams(
                  id,
                  name,
                  abbreviation,
                  country_code,
                  confederation,
                  flag_url_template,
                  fifa_rank,
                  fifa_ranking_points,
                  updated_at
                )
                values(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                on conflict(id) do update set
                  name = excluded.name,
                  abbreviation = excluded.abbreviation,
                  country_code = excluded.country_code,
                  confederation = excluded.confederation,
                  flag_url_template = excluded.flag_url_template,
                  fifa_rank = excluded.fifa_rank,
                  fifa_ranking_points = excluded.fifa_ranking_points,
                  updated_at = excluded.updated_at
                ",
                params![
                    team.id.as_str(),
                    team.name,
                    team.abbreviation,
                    team.country_code,
                    team.confederation.code(),
                    team.flag_url_template,
                    team.fifa_rank,
                    team.fifa_ranking_points,
                ],
            )
            .with_context(|| format!("saving team {}", team.id))?;
    }

    Ok(())
}

fn save_matches(connection: &Connection, matches: &[Match]) -> Result<()> {
    for match_ in matches {
        connection
            .execute(
                "
                insert into matches(
                  id,
                  match_number,
                  stage_id,
                  group_id,
                  utc_start,
                  local_start,
                  home_team_id,
                  away_team_id,
                  home_team_name,
                  away_team_name,
                  home_score,
                  away_score,
                  home_penalty_score,
                  away_penalty_score,
                  status,
                  minute,
                  stadium_name,
                  attendance,
                  winner_team_id,
                  raw_jsonb,
                  updated_at
                )
                values(
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                  ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19,
                  jsonb(?20),
                  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                )
                on conflict(id) do update set
                  match_number = excluded.match_number,
                  stage_id = excluded.stage_id,
                  group_id = excluded.group_id,
                  utc_start = excluded.utc_start,
                  local_start = excluded.local_start,
                  home_team_id = excluded.home_team_id,
                  away_team_id = excluded.away_team_id,
                  home_team_name = excluded.home_team_name,
                  away_team_name = excluded.away_team_name,
                  home_score = excluded.home_score,
                  away_score = excluded.away_score,
                  home_penalty_score = excluded.home_penalty_score,
                  away_penalty_score = excluded.away_penalty_score,
                  status = excluded.status,
                  minute = excluded.minute,
                  stadium_name = excluded.stadium_name,
                  attendance = excluded.attendance,
                  winner_team_id = excluded.winner_team_id,
                  raw_jsonb = excluded.raw_jsonb,
                  updated_at = excluded.updated_at
                ",
                params![
                    match_.id.as_str(),
                    match_.match_number,
                    match_.stage_id.as_str(),
                    match_.group_id.as_ref().map(GroupId::as_str),
                    match_.utc_start.to_string(),
                    match_.local_start.as_ref().map(Zoned::to_string),
                    match_.home_team_id.as_ref().map(TeamId::as_str),
                    match_.away_team_id.as_ref().map(TeamId::as_str),
                    match_.home_team_name,
                    match_.away_team_name,
                    match_.home_score,
                    match_.away_score,
                    match_.home_penalty_score,
                    match_.away_penalty_score,
                    match_status_to_storage(match_.status),
                    match_.minute,
                    match_.stadium_name,
                    match_.attendance,
                    match_.winner_team_id.as_ref().map(TeamId::as_str),
                    minimal_match_json(match_),
                ],
            )
            .with_context(|| format!("saving match {}", match_.id))?;
    }

    Ok(())
}

fn save_standings(connection: &Connection, standings: &[StandingRow]) -> Result<()> {
    let mut group_ids = standings
        .iter()
        .map(|row| row.group_id.as_str())
        .collect::<Vec<_>>();
    group_ids.sort_unstable();
    group_ids.dedup();

    let first_stage_id = Stage::FirstStage.id().to_string();
    for group_id in group_ids {
        connection
            .execute(
                "
                delete from standings
                where season_id = ?1 and stage_id = ?2 and group_id = ?3
                ",
                params![WORLD_CUP_2026.season_id, first_stage_id, group_id],
            )
            .with_context(|| format!("clearing standings for group {group_id}"))?;
    }

    for row in standings {
        connection
            .execute(
                "
                insert into standings(
                  season_id,
                  stage_id,
                  group_id,
                  team_id,
                  position,
                  played,
                  won,
                  drawn,
                  lost,
                  goals_for,
                  goals_against,
                  goal_difference,
                  points,
                  fair_play,
                  qualification_status,
                  raw_jsonb,
                  updated_at
                )
                values(
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                  ?11, ?12, ?13, ?14, ?15, jsonb(?16),
                  strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                )
                on conflict(season_id, stage_id, group_id, team_id) do update set
                  position = excluded.position,
                  played = excluded.played,
                  won = excluded.won,
                  drawn = excluded.drawn,
                  lost = excluded.lost,
                  goals_for = excluded.goals_for,
                  goals_against = excluded.goals_against,
                  goal_difference = excluded.goal_difference,
                  points = excluded.points,
                  fair_play = excluded.fair_play,
                  qualification_status = excluded.qualification_status,
                  raw_jsonb = excluded.raw_jsonb,
                  updated_at = excluded.updated_at
                ",
                params![
                    WORLD_CUP_2026.season_id,
                    first_stage_id,
                    row.group_id.as_str(),
                    row.team_id.as_str(),
                    row.position,
                    row.played,
                    row.won,
                    row.drawn,
                    row.lost,
                    row.goals_for,
                    row.goals_against,
                    row.goal_difference,
                    row.points,
                    row.fair_play,
                    row.qualification_status,
                    minimal_standing_json(row),
                ],
            )
            .with_context(|| format!("saving standings row for {}", row.team_id))?;
    }

    Ok(())
}

fn save_timeline_events(connection: &Connection, events: &[TimelineEvent]) -> Result<()> {
    let mut match_ids = events
        .iter()
        .map(|event| event.match_id.as_str())
        .collect::<Vec<_>>();
    match_ids.sort_unstable();
    match_ids.dedup();

    for match_id in match_ids {
        connection
            .execute(
                "delete from timeline_events where match_id = ?1",
                params![match_id],
            )
            .with_context(|| format!("clearing timeline events for match {match_id}"))?;
    }

    for event in events {
        connection
            .execute(
                "
                insert into timeline_events(
                  match_id,
                  event_index,
                  event_type,
                  team_id,
                  player_id,
                  minute,
                  description,
                  raw_jsonb
                )
                values(?1, ?2, ?3, ?4, ?5, ?6, ?7, jsonb(?8))
                ",
                params![
                    event.match_id.as_str(),
                    event.event_index,
                    event.event_type,
                    event.team_id.as_ref().map(TeamId::as_str),
                    event.player_id,
                    event.minute,
                    event.description,
                    minimal_timeline_json(event),
                ],
            )
            .with_context(|| {
                format!(
                    "saving timeline event {} for match {}",
                    event.event_index, event.match_id
                )
            })?;
    }

    Ok(())
}

fn parse_timestamp(value: &str, field: &str) -> Result<Timestamp> {
    value
        .parse::<Timestamp>()
        .with_context(|| format!("invalid timestamp in {field}: {value}"))
}

fn parse_optional_zoned(value: Option<&str>) -> Result<Option<Zoned>> {
    value
        .map(str::parse::<Zoned>)
        .transpose()
        .context("invalid zoned local_start timestamp")
}

fn match_status_to_storage(status: MatchStatus) -> u16 {
    match status {
        MatchStatus::Scheduled => 1,
        MatchStatus::Live => 3,
        MatchStatus::FullTime => 0,
        MatchStatus::ExtraTime => 4,
        MatchStatus::Penalties => 5,
        MatchStatus::Postponed => 6,
        MatchStatus::Cancelled => 7,
        MatchStatus::Unknown(value) => value,
    }
}

fn match_status_from_storage(value: u16) -> MatchStatus {
    match value {
        1 => MatchStatus::Scheduled,
        3 => MatchStatus::Live,
        0 => MatchStatus::FullTime,
        4 => MatchStatus::ExtraTime,
        5 => MatchStatus::Penalties,
        6 => MatchStatus::Postponed,
        7 => MatchStatus::Cancelled,
        value => MatchStatus::Unknown(value),
    }
}

fn minimal_match_json(match_: &Match) -> String {
    serde_json::json!({
        "id": match_.id.as_str(),
        "match_number": match_.match_number,
        "source": "mapped"
    })
    .to_string()
}

fn minimal_timeline_json(event: &TimelineEvent) -> String {
    serde_json::json!({
        "match_id": event.match_id.as_str(),
        "event_index": event.event_index,
        "event_type": event.event_type,
        "source": "mapped"
    })
    .to_string()
}

fn minimal_standing_json(row: &StandingRow) -> String {
    serde_json::json!({
        "group_id": row.group_id.as_str(),
        "team_id": row.team_id.as_str(),
        "position": row.position,
        "source": "mapped"
    })
    .to_string()
}

fn apply_migrations(connection: &mut Connection) -> Result<usize> {
    connection
        .execute_batch(
            "
            create table if not exists schema_migrations(
              version text primary key,
              applied_at text not null
            ) strict, without rowid;
            ",
        )
        .context("creating schema_migrations table")?;

    let transaction = connection
        .transaction()
        .context("starting migration transaction")?;
    let mut applied = 0;

    for migration in MIGRATIONS {
        let already_applied: bool = transaction
            .query_row(
                "select exists(select 1 from schema_migrations where version = ?1)",
                [migration.version],
                |row| row.get(0),
            )
            .with_context(|| format!("checking migration {}", migration.version))?;

        if already_applied {
            continue;
        }

        transaction
            .execute_batch(migration.sql)
            .with_context(|| format!("applying migration {}", migration.version))?;
        transaction
            .execute(
                "insert into schema_migrations(version, applied_at)
                 values(?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                [migration.version],
            )
            .with_context(|| format!("recording migration {}", migration.version))?;
        applied += 1;
    }

    transaction.commit().context("committing migrations")?;
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Confederation, StageId};

    #[test]
    fn bundled_sqlite_supports_jsonb_requirement() {
        assert!(rusqlite::version_number() >= MIN_SQLITE_VERSION);
    }

    #[test]
    fn migrations_are_sorted_and_unique() {
        let mut previous = "";

        for migration in MIGRATIONS {
            assert!(migration.version > previous);
            previous = migration.version;
        }
    }

    #[test]
    fn applies_migrations_to_empty_database() {
        let mut connection = Connection::open_in_memory().expect("in-memory database");
        configure_connection(&connection).expect("connection config");

        let applied = apply_migrations(&mut connection).expect("apply migrations");
        assert_eq!(applied, MIGRATIONS.len());

        let table_count: i64 = connection
            .query_row(
                "select count(*) from sqlite_schema where type = 'table' and name in (
                    'schema_migrations',
                    'cache_entries',
                    'teams',
                    'favorite_teams',
                    'stages',
                    'groups',
                    'matches',
                    'standings',
                    'player_stats',
                    'timeline_events',
                    'sync_runs',
                    'resource_refreshes'
                )",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(table_count, 12);

        let applied_again = apply_migrations(&mut connection).expect("apply migrations again");
        assert_eq!(applied_again, 0);
    }

    #[test]
    fn debug_database_path_is_project_local() {
        let (path, location) = database_path().expect("database path");

        if cfg!(debug_assertions) {
            assert_eq!(location, DbLocation::ProjectLocal);
            assert!(path.ends_with(".footui/footui.sqlite3"));
        }
    }

    #[test]
    fn repository_round_trips_teams_and_matches() {
        let db_path = temp_db_path("round-trip");
        initialize_at(db_path.clone(), DbLocation::ProjectLocal).expect("db init");
        let repository = SqliteRepository::new(db_path.clone());

        repository
            .save_sync_result(SyncResult {
                teams: vec![sample_team(
                    "43911",
                    "Mexico",
                    "MEX",
                    Confederation::Concacaf,
                )],
                matches: vec![sample_match()],
                standings: vec![sample_standing()],
                timeline_events: vec![sample_timeline_event()],
                ..SyncResult::default()
            })
            .expect("save sync result");

        let snapshot = repository.load_snapshot().expect("snapshot");

        assert_eq!(snapshot.teams.len(), 1);
        assert_eq!(snapshot.teams[0].name, "Mexico");
        assert_eq!(snapshot.teams[0].fifa_rank, Some(14));
        assert_eq!(snapshot.teams[0].fifa_ranking_points, Some(1682.4));
        assert_eq!(snapshot.matches.len(), 1);
        assert_eq!(snapshot.matches[0].home_team_name, "Mexico");
        assert_eq!(snapshot.matches[0].status, MatchStatus::Scheduled);
        assert_eq!(snapshot.standings.len(), 1);
        assert_eq!(snapshot.standings[0].team_name, "Mexico");
        assert_eq!(snapshot.standings[0].fair_play, Some(-4));
        assert_eq!(snapshot.timeline_events.len(), 1);
        assert_eq!(snapshot.timeline_events[0].event_type, Some(0));
        assert!(
            latest_data_updated_at(&db_path)
                .expect("latest updated at")
                .is_some()
        );

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn repository_toggles_favorite_team() {
        let db_path = temp_db_path("favorite-toggle");
        initialize_at(db_path.clone(), DbLocation::ProjectLocal).expect("db init");
        let repository = SqliteRepository::new(db_path.clone());

        repository
            .save_sync_result(SyncResult {
                teams: vec![sample_team(
                    "43911",
                    "Mexico",
                    "MEX",
                    Confederation::Concacaf,
                )],
                ..SyncResult::default()
            })
            .expect("save sync result");

        repository
            .toggle_favorite_team(TeamId::from("43911"))
            .expect("toggle on");
        assert!(repository.load_snapshot().expect("snapshot").teams[0].favorite);

        repository
            .toggle_favorite_team(TeamId::from("43911"))
            .expect("toggle off");
        assert!(!repository.load_snapshot().expect("snapshot").teams[0].favorite);

        let _ = fs::remove_file(db_path);
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
            fifa_rank: Some(14),
            fifa_ranking_points: Some(1682.4),
            favorite: false,
        }
    }

    fn sample_match() -> Match {
        Match {
            id: MatchId::from("400021443"),
            match_number: 1,
            stage_id: StageId::from("289273"),
            stage_name: "First Stage".to_string(),
            group_id: Some(GroupId::from("289275")),
            group_name: Some("Group A".to_string()),
            utc_start: "2026-06-11T19:00:00Z".parse().expect("timestamp"),
            local_start: None,
            home_team_id: Some(TeamId::from("43911")),
            away_team_id: None,
            home_team_name: "Mexico".to_string(),
            away_team_name: "TBD".to_string(),
            home_score: None,
            away_score: None,
            home_penalty_score: None,
            away_penalty_score: None,
            status: MatchStatus::Scheduled,
            minute: None,
            stadium_name: Some("Mexico City Stadium".to_string()),
            attendance: None,
            winner_team_id: None,
        }
    }

    fn sample_timeline_event() -> TimelineEvent {
        TimelineEvent {
            match_id: MatchId::from("400021443"),
            event_index: 1,
            event_type: Some(0),
            team_id: Some(TeamId::from("43911")),
            player_id: Some("429157".to_string()),
            minute: Some("9'".to_string()),
            description: Some("Mexico scores.".to_string()),
        }
    }

    fn sample_standing() -> StandingRow {
        StandingRow {
            group_id: GroupId::from("289275"),
            group_name: "Group A".to_string(),
            team_id: TeamId::from("43911"),
            team_name: "Mexico".to_string(),
            position: 1,
            played: 1,
            won: 1,
            drawn: 0,
            lost: 0,
            goals_for: 2,
            goals_against: 0,
            goal_difference: 2,
            points: 3,
            qualification_status: Some("qualified".to_string()),
            fair_play: Some(-4),
        }
    }

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "footui-sqlite-{name}-{}.sqlite3",
            std::process::id()
        ))
    }
}
