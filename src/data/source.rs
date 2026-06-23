use std::future::Future;

use anyhow::Result;

use crate::domain::{GroupId, Match, MatchId, PlayerStat, Stage, StandingRow, Team, TimelineEvent};

pub trait DataSource {
    fn teams(&self) -> impl Future<Output = Result<Vec<Team>>> + Send;

    fn stages(&self) -> impl Future<Output = Result<Vec<Stage>>> + Send;

    fn matches(&self) -> impl Future<Output = Result<Vec<Match>>> + Send;

    fn standings(
        &self,
        group: Option<GroupId>,
    ) -> impl Future<Output = Result<Vec<StandingRow>>> + Send;

    fn top_scorers(&self) -> impl Future<Output = Result<Vec<PlayerStat>>> + Send;

    fn timeline(
        &self,
        match_id: MatchId,
    ) -> impl Future<Output = Result<Vec<TimelineEvent>>> + Send;
}
