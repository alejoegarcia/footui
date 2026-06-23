use crate::domain::TeamId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerStat {
    pub player_id: String,
    pub player_name: String,
    pub team_id: TeamId,
    pub country_code: String,
    pub rank: u16,
    pub matches_played: u16,
    pub minutes_played: u16,
    pub goals: u16,
    pub assists: u16,
    pub attempts: u16,
    pub attempts_on_target: u16,
}
