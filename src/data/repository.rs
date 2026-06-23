use anyhow::Result;

use crate::domain::{Match, PlayerStat, StandingRow, Team, TeamId, TimelineEvent};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AppSnapshot {
    pub teams: Vec<Team>,
    pub matches: Vec<Match>,
    pub standings: Vec<StandingRow>,
    pub top_scorers: Vec<PlayerStat>,
    pub timeline_events: Vec<TimelineEvent>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncResult {
    pub teams: Vec<Team>,
    pub matches: Vec<Match>,
    pub standings: Vec<StandingRow>,
    pub top_scorers: Vec<PlayerStat>,
    pub timeline_events: Vec<TimelineEvent>,
}

pub trait Repository {
    fn load_snapshot(&self) -> Result<AppSnapshot>;
    fn save_sync_result(&self, result: SyncResult) -> Result<()>;
    fn toggle_favorite_team(&self, team_id: TeamId) -> Result<()>;
}
