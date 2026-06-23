use crate::domain::{GroupId, TeamId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandingRow {
    pub group_id: GroupId,
    pub group_name: String,
    pub team_id: TeamId,
    pub team_name: String,
    pub position: u8,
    pub played: u8,
    pub won: u8,
    pub drawn: u8,
    pub lost: u8,
    pub goals_for: i16,
    pub goals_against: i16,
    pub goal_difference: i16,
    pub points: i16,
    pub qualification_status: Option<String>,
    pub fair_play: Option<i16>,
}

impl StandingRow {
    pub fn qualification_state(&self) -> QualificationState {
        QualificationState::from_fifa_status(self.qualification_status.as_deref())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationState {
    Open,
    Qualified,
    Disqualified,
}

impl QualificationState {
    pub fn from_fifa_status(status: Option<&str>) -> Self {
        match status.map(|status| status.to_ascii_lowercase()).as_deref() {
            Some("confirmedqualified" | "qualified") => Self::Qualified,
            Some(
                "confirmeddisqualified"
                | "disqualified"
                | "confirmedeliminated"
                | "eliminated"
                | "confirmednotqualified"
                | "notqualified",
            ) => Self::Disqualified,
            _ => Self::Open,
        }
    }

    pub fn is_qualified(self) -> bool {
        self == Self::Qualified
    }

    pub fn is_disqualified(self) -> bool {
        self == Self::Disqualified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_fifa_qualification_status_to_single_state() {
        assert_eq!(
            QualificationState::from_fifa_status(Some("ConfirmedQualified")),
            QualificationState::Qualified
        );
        assert_eq!(
            QualificationState::from_fifa_status(Some("ConfirmedDisqualified")),
            QualificationState::Disqualified
        );
        assert_eq!(
            QualificationState::from_fifa_status(Some("CouldQualify")),
            QualificationState::Open
        );
        assert_eq!(
            QualificationState::from_fifa_status(None),
            QualificationState::Open
        );
    }
}
