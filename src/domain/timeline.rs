use crate::domain::{MatchId, TeamId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimelineEvent {
    pub match_id: MatchId,
    pub event_index: u16,
    pub event_type: Option<u16>,
    pub team_id: Option<TeamId>,
    pub player_id: Option<String>,
    pub minute: Option<String>,
    pub description: Option<String>,
}
