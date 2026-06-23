#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TournamentConfig {
    pub competition_id: &'static str,
    pub season_id: &'static str,
    pub name: &'static str,
}

pub const WORLD_CUP_2026: TournamentConfig = TournamentConfig {
    competition_id: "17",
    season_id: "285023",
    name: "FIFA World Cup 2026 TUI",
};
