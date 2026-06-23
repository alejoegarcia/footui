use jiff::{SignedDuration, Timestamp};

use crate::data::sync::ResourceKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshPolicyReason {
    Forced,
    NeverRefreshed,
    Stale,
    Fresh,
    WaitingForRelevantMatchWindow,
    Immutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshPolicyDecision {
    pub should_refresh: bool,
    pub next_refresh_after: Option<Timestamp>,
    pub reason: RefreshPolicyReason,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScheduleContext {
    pub tournament_started: bool,
    pub tournament_finished: bool,
    pub next_match_at: Option<Timestamp>,
    pub active_match_window: bool,
    pub resource_next_match_at: Option<Timestamp>,
    pub resource_recent_match_at: Option<Timestamp>,
    pub resource_live: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshPolicyInput {
    pub resource: ResourceKey,
    pub now: Timestamp,
    pub last_success_at: Option<Timestamp>,
    pub force: bool,
    pub schedule: ScheduleContext,
}

pub fn decide(input: RefreshPolicyInput) -> RefreshPolicyDecision {
    if input.force {
        return RefreshPolicyDecision {
            should_refresh: true,
            next_refresh_after: None,
            reason: RefreshPolicyReason::Forced,
        };
    }

    let Some(last_success_at) = input.last_success_at else {
        return RefreshPolicyDecision {
            should_refresh: true,
            next_refresh_after: Some(input.now),
            reason: RefreshPolicyReason::NeverRefreshed,
        };
    };

    match input.resource {
        ResourceKey::Teams => fixed_interval(input.now, last_success_at, days(7)),
        ResourceKey::Stages => fixed_interval(input.now, last_success_at, days(30)),
        ResourceKey::Matches => matches_policy(input.now, last_success_at, &input.schedule),
        ResourceKey::StandingsGroup(_) => {
            standings_policy(input.now, last_success_at, &input.schedule)
        }
        ResourceKey::TopScorers => top_scorers_policy(input.now, last_success_at, &input.schedule),
        ResourceKey::Timeline(_) => timeline_policy(input.now, last_success_at, &input.schedule),
    }
}

pub fn next_after_success(
    resource: &ResourceKey,
    now: Timestamp,
    schedule: &ScheduleContext,
) -> Option<Timestamp> {
    let input = RefreshPolicyInput {
        resource: resource.clone(),
        now,
        last_success_at: Some(now),
        force: false,
        schedule: schedule.clone(),
    };
    decide(input).next_refresh_after
}

fn matches_policy(
    now: Timestamp,
    last_success_at: Timestamp,
    schedule: &ScheduleContext,
) -> RefreshPolicyDecision {
    if schedule.active_match_window {
        return interval_decision(now, last_success_at, seconds(60));
    }

    if let Some(next_match_at) = schedule.next_match_at {
        let pre_match_refresh = add(next_match_at, -hours(2));
        if now < pre_match_refresh {
            return RefreshPolicyDecision {
                should_refresh: false,
                next_refresh_after: Some(pre_match_refresh),
                reason: RefreshPolicyReason::WaitingForRelevantMatchWindow,
            };
        }

        return interval_decision(now, last_success_at, minutes(5));
    }

    fixed_interval(now, last_success_at, hours(12))
}

fn standings_policy(
    now: Timestamp,
    last_success_at: Timestamp,
    schedule: &ScheduleContext,
) -> RefreshPolicyDecision {
    if schedule.resource_live {
        return interval_decision(now, last_success_at, seconds(60));
    }

    if schedule.resource_recent_match_at.is_some() {
        return interval_decision(now, last_success_at, minutes(5));
    }

    if let Some(next_match_at) = schedule.resource_next_match_at {
        let pre_match_refresh = add(next_match_at, -hours(2));
        if now < pre_match_refresh {
            return RefreshPolicyDecision {
                should_refresh: false,
                next_refresh_after: Some(pre_match_refresh),
                reason: RefreshPolicyReason::WaitingForRelevantMatchWindow,
            };
        }

        return interval_decision(now, last_success_at, minutes(5));
    }

    if schedule.tournament_finished {
        return RefreshPolicyDecision {
            should_refresh: false,
            next_refresh_after: None,
            reason: RefreshPolicyReason::Immutable,
        };
    }

    fixed_interval(now, last_success_at, hours(24))
}

fn top_scorers_policy(
    now: Timestamp,
    last_success_at: Timestamp,
    schedule: &ScheduleContext,
) -> RefreshPolicyDecision {
    if schedule.active_match_window {
        interval_decision(now, last_success_at, minutes(5))
    } else {
        fixed_interval(now, last_success_at, hours(12))
    }
}

fn timeline_policy(
    now: Timestamp,
    last_success_at: Timestamp,
    schedule: &ScheduleContext,
) -> RefreshPolicyDecision {
    if schedule.resource_live {
        return interval_decision(now, last_success_at, seconds(60));
    }

    if schedule.resource_recent_match_at.is_some() {
        return interval_decision(now, last_success_at, minutes(5));
    }

    RefreshPolicyDecision {
        should_refresh: false,
        next_refresh_after: None,
        reason: RefreshPolicyReason::Immutable,
    }
}

fn fixed_interval(
    now: Timestamp,
    last_success_at: Timestamp,
    interval: SignedDuration,
) -> RefreshPolicyDecision {
    interval_decision(now, last_success_at, interval)
}

fn interval_decision(
    now: Timestamp,
    last_success_at: Timestamp,
    interval: SignedDuration,
) -> RefreshPolicyDecision {
    let next_refresh_after = add(last_success_at, interval);
    let should_refresh = now >= next_refresh_after;

    RefreshPolicyDecision {
        should_refresh,
        next_refresh_after: Some(next_refresh_after),
        reason: if should_refresh {
            RefreshPolicyReason::Stale
        } else {
            RefreshPolicyReason::Fresh
        },
    }
}

fn add(timestamp: Timestamp, duration: SignedDuration) -> Timestamp {
    timestamp
        .checked_add(duration)
        .expect("refresh policy timestamp arithmetic should stay in range")
}

fn seconds(value: i64) -> SignedDuration {
    SignedDuration::new(value, 0)
}

fn minutes(value: i64) -> SignedDuration {
    seconds(value * 60)
}

fn hours(value: i64) -> SignedDuration {
    minutes(value * 60)
}

fn days(value: i64) -> SignedDuration {
    hours(value * 24)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{data::sync::ResourceKey, domain::GroupId};

    fn ts(seconds: i64) -> Timestamp {
        Timestamp::new(seconds, 0).expect("timestamp")
    }

    #[test]
    fn standings_wait_until_relevant_group_window() {
        let now = ts(1_000);
        let next_group_match = ts(1_000 + 7 * 24 * 60 * 60);
        let decision = decide(RefreshPolicyInput {
            resource: ResourceKey::StandingsGroup(GroupId::from("289275")),
            now,
            last_success_at: Some(now),
            force: false,
            schedule: ScheduleContext {
                tournament_started: true,
                resource_next_match_at: Some(next_group_match),
                ..ScheduleContext::default()
            },
        });

        assert!(!decision.should_refresh);
        assert_eq!(
            decision.reason,
            RefreshPolicyReason::WaitingForRelevantMatchWindow
        );
        assert_eq!(
            decision.next_refresh_after,
            Some(add(next_group_match, -hours(2)))
        );
    }

    #[test]
    fn live_timeline_refreshes_every_minute() {
        let now = ts(1_000);
        let decision = decide(RefreshPolicyInput {
            resource: ResourceKey::Timeline("400000001".into()),
            now,
            last_success_at: Some(ts(930)),
            force: false,
            schedule: ScheduleContext {
                resource_live: true,
                ..ScheduleContext::default()
            },
        });

        assert!(decision.should_refresh);
        assert_eq!(decision.next_refresh_after, Some(ts(990)));
    }

    #[test]
    fn teams_are_static_after_tournament_start() {
        let now = ts(1_000);
        let decision = decide(RefreshPolicyInput {
            resource: ResourceKey::Teams,
            now,
            last_success_at: Some(ts(1_000 - 24 * 60 * 60)),
            force: false,
            schedule: ScheduleContext {
                tournament_started: true,
                ..ScheduleContext::default()
            },
        });

        assert!(!decision.should_refresh);
        assert_eq!(decision.reason, RefreshPolicyReason::Fresh);
    }
}
