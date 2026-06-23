use anyhow::{Result, bail};

use crate::domain::{GroupId, StageId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Confederation {
    Afc,
    Caf,
    Concacaf,
    Conmebol,
    Ofc,
    Uefa,
}

impl Confederation {
    pub const ALL: [Self; 6] = [
        Self::Afc,
        Self::Caf,
        Self::Concacaf,
        Self::Conmebol,
        Self::Ofc,
        Self::Uefa,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Afc => "AFC",
            Self::Caf => "CAF",
            Self::Concacaf => "CONCACAF",
            Self::Conmebol => "CONMEBOL",
            Self::Ofc => "OFC",
            Self::Uefa => "UEFA",
        }
    }

    pub fn from_code(value: &str) -> Result<Self> {
        match value {
            "AFC" => Ok(Self::Afc),
            "CAF" => Ok(Self::Caf),
            "CONCACAF" => Ok(Self::Concacaf),
            "CONMEBOL" => Ok(Self::Conmebol),
            "OFC" => Ok(Self::Ofc),
            "UEFA" => Ok(Self::Uefa),
            _ => bail!("unknown confederation code: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Stage {
    FirstStage,
    RoundOf32,
    RoundOf16,
    QuarterFinal,
    SemiFinal,
    ThirdPlace,
    Final,
}

impl Stage {
    pub const ALL: [Self; 7] = [
        Self::FirstStage,
        Self::RoundOf32,
        Self::RoundOf16,
        Self::QuarterFinal,
        Self::SemiFinal,
        Self::ThirdPlace,
        Self::Final,
    ];

    pub fn id(self) -> StageId {
        StageId::from(match self {
            Self::FirstStage => "289273",
            Self::RoundOf32 => "289287",
            Self::RoundOf16 => "289288",
            Self::QuarterFinal => "289289",
            Self::SemiFinal => "289290",
            Self::ThirdPlace => "289291",
            Self::Final => "289292",
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::FirstStage => "First Stage",
            Self::RoundOf32 => "Round of 32",
            Self::RoundOf16 => "Round of 16",
            Self::QuarterFinal => "Quarter-final",
            Self::SemiFinal => "Semi-final",
            Self::ThirdPlace => "Play-off for third place",
            Self::Final => "Final",
        }
    }

    pub fn sort_order(self) -> u8 {
        match self {
            Self::FirstStage => 1,
            Self::RoundOf32 => 2,
            Self::RoundOf16 => 3,
            Self::QuarterFinal => 4,
            Self::SemiFinal => 5,
            Self::ThirdPlace => 6,
            Self::Final => 7,
        }
    }

    pub fn from_id(value: &str) -> Result<Self> {
        match value {
            "289273" => Ok(Self::FirstStage),
            "289287" => Ok(Self::RoundOf32),
            "289288" => Ok(Self::RoundOf16),
            "289289" => Ok(Self::QuarterFinal),
            "289290" => Ok(Self::SemiFinal),
            "289291" => Ok(Self::ThirdPlace),
            "289292" => Ok(Self::Final),
            _ => bail!("unknown stage id: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Group {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
}

impl Group {
    pub const ALL: [Self; 12] = [
        Self::A,
        Self::B,
        Self::C,
        Self::D,
        Self::E,
        Self::F,
        Self::G,
        Self::H,
        Self::I,
        Self::J,
        Self::K,
        Self::L,
    ];

    pub fn id(self) -> GroupId {
        GroupId::from(match self {
            Self::A => "289275",
            Self::B => "289276",
            Self::C => "289277",
            Self::D => "289278",
            Self::E => "289279",
            Self::F => "289280",
            Self::G => "289281",
            Self::H => "289282",
            Self::I => "289283",
            Self::J => "289284",
            Self::K => "289285",
            Self::L => "289286",
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::A => "Group A",
            Self::B => "Group B",
            Self::C => "Group C",
            Self::D => "Group D",
            Self::E => "Group E",
            Self::F => "Group F",
            Self::G => "Group G",
            Self::H => "Group H",
            Self::I => "Group I",
            Self::J => "Group J",
            Self::K => "Group K",
            Self::L => "Group L",
        }
    }

    pub fn from_id(value: &str) -> Result<Self> {
        match value {
            "289275" => Ok(Self::A),
            "289276" => Ok(Self::B),
            "289277" => Ok(Self::C),
            "289278" => Ok(Self::D),
            "289279" => Ok(Self::E),
            "289280" => Ok(Self::F),
            "289281" => Ok(Self::G),
            "289282" => Ok(Self::H),
            "289283" => Ok(Self::I),
            "289284" => Ok(Self::J),
            "289285" => Ok(Self::K),
            "289286" => Ok(Self::L),
            _ => bail!("unknown group id: {value}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_values_match_sqlite_constraints() {
        assert_eq!(
            Confederation::ALL.map(Confederation::code),
            ["AFC", "CAF", "CONCACAF", "CONMEBOL", "OFC", "UEFA"]
        );
        assert_eq!(
            Stage::ALL.map(|stage| stage.id().to_string()),
            [
                "289273", "289287", "289288", "289289", "289290", "289291", "289292"
            ]
        );
        assert_eq!(
            Group::ALL.map(|group| group.id().to_string()),
            [
                "289275", "289276", "289277", "289278", "289279", "289280", "289281", "289282",
                "289283", "289284", "289285", "289286"
            ]
        );
    }

    #[test]
    fn rejects_unknown_fixed_tournament_values() {
        assert!(Confederation::from_code("XYZ").is_err());
        assert!(Stage::from_id("0").is_err());
        assert!(Group::from_id("0").is_err());
    }
}
