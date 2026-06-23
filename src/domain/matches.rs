use jiff::{Timestamp, Zoned};

use crate::domain::{GroupId, MatchId, StageId, TeamId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MatchStatus {
    Scheduled,
    Live,
    FullTime,
    ExtraTime,
    Penalties,
    Postponed,
    Cancelled,
    Unknown(u16),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub id: MatchId,
    pub match_number: u16,
    pub stage_id: StageId,
    pub stage_name: String,
    pub group_id: Option<GroupId>,
    pub group_name: Option<String>,
    pub utc_start: Timestamp,
    pub local_start: Option<Zoned>,
    pub home_team_id: Option<TeamId>,
    pub away_team_id: Option<TeamId>,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_score: Option<u8>,
    pub away_score: Option<u8>,
    pub home_penalty_score: Option<u8>,
    pub away_penalty_score: Option<u8>,
    pub status: MatchStatus,
    pub minute: Option<String>,
    pub stadium_name: Option<String>,
    pub attendance: Option<u32>,
    pub winner_team_id: Option<TeamId>,
}
