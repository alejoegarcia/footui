use std::fmt;

use crate::domain::{Group, GroupId, MatchId};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResourceKey {
    Teams,
    Stages,
    Matches,
    StandingsGroup(GroupId),
    TopScorers,
    Timeline(MatchId),
}

impl ResourceKey {
    pub fn all_standings_groups() -> Vec<Self> {
        Group::ALL
            .iter()
            .map(|group| Self::StandingsGroup(group.id()))
            .collect()
    }

    pub fn requires_network(&self) -> bool {
        matches!(
            self,
            Self::Teams | Self::Matches | Self::StandingsGroup(_) | Self::Timeline(_)
        )
    }

    pub fn storage_key(&self) -> String {
        match self {
            Self::Teams => "teams".to_string(),
            Self::Stages => "stages".to_string(),
            Self::Matches => "matches".to_string(),
            Self::StandingsGroup(group_id) => format!("standings:group:{group_id}"),
            Self::TopScorers => "top_scorers".to_string(),
            Self::Timeline(match_id) => format!("timeline:{match_id}"),
        }
    }

    pub fn resource_type(&self) -> &'static str {
        match self {
            Self::Teams => "teams",
            Self::Stages => "stages",
            Self::Matches => "matches",
            Self::StandingsGroup(_) => "standings_group",
            Self::TopScorers => "top_scorers",
            Self::Timeline(_) => "timeline",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Teams => "teams".to_string(),
            Self::Stages => "stages".to_string(),
            Self::Matches => "matches".to_string(),
            Self::StandingsGroup(group_id) => format!("standings {group_id}"),
            Self::TopScorers => "top scorers".to_string(),
            Self::Timeline(match_id) => format!("timeline {match_id}"),
        }
    }
}

impl fmt::Display for ResourceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.storage_key())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_keys_are_stable_storage_strings() {
        assert_eq!(ResourceKey::Teams.storage_key(), "teams");
        assert_eq!(
            ResourceKey::StandingsGroup(GroupId::from("289275")).storage_key(),
            "standings:group:289275"
        );
        assert_eq!(
            ResourceKey::Timeline(MatchId::from("400000001")).storage_key(),
            "timeline:400000001"
        );
    }

    #[test]
    fn standings_resource_expands_to_all_twelve_groups() {
        assert_eq!(ResourceKey::all_standings_groups().len(), 12);
        assert_eq!(
            ResourceKey::all_standings_groups()[0].storage_key(),
            "standings:group:289275"
        );
    }
}
