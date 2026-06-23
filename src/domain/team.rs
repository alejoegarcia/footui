use crate::domain::{Confederation, TeamId};

#[derive(Clone, Debug, PartialEq)]
pub struct Team {
    pub id: TeamId,
    pub name: String,
    pub abbreviation: String,
    pub country_code: String,
    pub confederation: Confederation,
    pub flag_url_template: Option<String>,
    pub fifa_rank: Option<u16>,
    pub fifa_ranking_points: Option<f64>,
    pub favorite: bool,
}
