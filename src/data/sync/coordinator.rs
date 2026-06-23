use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use jiff::Timestamp;
use tokio::sync::mpsc;

use crate::{
    config::TournamentConfig,
    data::{
        fifa::{FifaDataSource, check_network_access},
        repository::{Repository, SyncResult},
        source::DataSource,
        sqlite::SqliteRepository,
        sync::{
            ResourceKey,
            metadata::{record_attempt, record_error, record_success},
            policy::{ScheduleContext, next_after_success},
        },
    },
};

const MANUAL_RESOURCE_COOLDOWN: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshReason {
    Startup,
    Manual,
    ScreenEnter,
    Policy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshRequest {
    pub resources: Vec<ResourceKey>,
    pub reason: RefreshReason,
    pub force: bool,
}

impl RefreshRequest {
    pub fn manual(resources: Vec<ResourceKey>) -> Self {
        Self {
            resources,
            reason: RefreshReason::Manual,
            force: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefreshEvent {
    Started {
        resource: ResourceKey,
    },
    Deduped {
        resource: ResourceKey,
    },
    Cooldown {
        resource: ResourceKey,
    },
    Succeeded {
        resource: ResourceKey,
        at: String,
    },
    Failed {
        resource: ResourceKey,
        error: String,
    },
    Offline {
        resource: ResourceKey,
        error: String,
    },
}

#[derive(Clone, Debug)]
pub struct RefreshCoordinator {
    sender: mpsc::UnboundedSender<RefreshRequest>,
}

impl RefreshCoordinator {
    pub fn start(
        db_path: PathBuf,
        config: TournamentConfig,
        event_sender: mpsc::UnboundedSender<RefreshEvent>,
    ) -> RefreshCoordinator {
        let (sender, receiver) = mpsc::unbounded_channel();
        tokio::spawn(run_coordinator(db_path, config, receiver, event_sender));
        RefreshCoordinator { sender }
    }

    pub fn request(&self, request: RefreshRequest) -> Result<()> {
        self.sender
            .send(request)
            .map_err(|_| anyhow!("refresh coordinator is not running"))
    }
}

async fn run_coordinator(
    db_path: PathBuf,
    config: TournamentConfig,
    mut receiver: mpsc::UnboundedReceiver<RefreshRequest>,
    event_sender: mpsc::UnboundedSender<RefreshEvent>,
) {
    let (done_sender, mut done_receiver) = mpsc::unbounded_channel();
    let mut in_flight = HashSet::new();
    let mut cooldown_until = HashMap::<ResourceKey, Instant>::new();

    loop {
        tokio::select! {
            Some(request) = receiver.recv() => {
                let now = Instant::now();
                let network_error = if request.resources.iter().any(ResourceKey::requires_network) {
                    check_network_access().await.err().map(|error| error.to_string())
                } else {
                    None
                };

                for resource in request.resources {
                    if in_flight.contains(&resource) {
                        let _ = event_sender.send(RefreshEvent::Deduped { resource });
                        continue;
                    }

                    if cooldown_until.get(&resource).is_some_and(|until| *until > now) {
                        let _ = event_sender.send(RefreshEvent::Cooldown { resource });
                        continue;
                    }

                    if resource.requires_network() {
                        if let Some(error) = network_error.as_ref() {
                            cooldown_until.insert(resource.clone(), now + MANUAL_RESOURCE_COOLDOWN);
                            let _ = event_sender.send(RefreshEvent::Offline {
                                resource,
                                error: error.clone(),
                            });
                            continue;
                        }
                    }

                    in_flight.insert(resource.clone());
                    cooldown_until.insert(resource.clone(), now + MANUAL_RESOURCE_COOLDOWN);
                    let _ = event_sender.send(RefreshEvent::Started {
                        resource: resource.clone(),
                    });

                    spawn_refresh(
                        db_path.clone(),
                        config,
                        resource,
                        event_sender.clone(),
                        done_sender.clone(),
                    );
                }
            }
            Some(resource) = done_receiver.recv() => {
                in_flight.remove(&resource);
            }
            else => break,
        }
    }
}

fn spawn_refresh(
    db_path: PathBuf,
    config: TournamentConfig,
    resource: ResourceKey,
    event_sender: mpsc::UnboundedSender<RefreshEvent>,
    done_sender: mpsc::UnboundedSender<ResourceKey>,
) {
    tokio::spawn(async move {
        let result = sync_resource(db_path.clone(), config, resource.clone()).await;

        match result {
            Ok(at) => {
                let _ = event_sender.send(RefreshEvent::Succeeded {
                    resource: resource.clone(),
                    at,
                });
            }
            Err(error) => {
                let message = error.to_string();
                let _ = record_error(&db_path, &resource, &message);
                let _ = event_sender.send(RefreshEvent::Failed {
                    resource: resource.clone(),
                    error: message,
                });
            }
        }

        let _ = done_sender.send(resource);
    });
}

async fn sync_resource(
    db_path: PathBuf,
    config: TournamentConfig,
    resource: ResourceKey,
) -> Result<String> {
    record_attempt_blocking(db_path.clone(), resource.clone()).await?;

    match &resource {
        ResourceKey::Teams => {
            let source = FifaDataSource::new(config)?;
            let teams = source.teams().await?;
            save_sync_result_blocking(
                db_path.clone(),
                SyncResult {
                    teams,
                    ..SyncResult::default()
                },
            )
            .await?;
        }
        ResourceKey::Matches => {
            let source = FifaDataSource::new(config)?;
            let (teams, matches) = tokio::try_join!(source.teams(), source.matches())?;
            save_sync_result_blocking(
                db_path.clone(),
                SyncResult {
                    teams,
                    matches,
                    ..SyncResult::default()
                },
            )
            .await?;
        }
        ResourceKey::Timeline(match_id) => {
            let source = FifaDataSource::new(config)?;
            let timeline_events = source.timeline(match_id.clone()).await?;
            save_sync_result_blocking(
                db_path.clone(),
                SyncResult {
                    timeline_events,
                    ..SyncResult::default()
                },
            )
            .await?;
        }
        ResourceKey::StandingsGroup(group_id) => {
            let source = FifaDataSource::new(config)?;
            let (teams, standings) =
                tokio::try_join!(source.teams(), source.standings(Some(group_id.clone())))?;
            save_sync_result_blocking(
                db_path.clone(),
                SyncResult {
                    teams,
                    standings,
                    ..SyncResult::default()
                },
            )
            .await?;
        }
        ResourceKey::Stages | ResourceKey::TopScorers => {}
    }

    let now = Timestamp::now();
    let next_refresh_after = next_after_success(&resource, now, &ScheduleContext::default());
    record_success_blocking(db_path, resource, next_refresh_after).await?;
    Ok(now.to_string())
}

async fn record_attempt_blocking(db_path: PathBuf, resource: ResourceKey) -> Result<()> {
    tokio::task::spawn_blocking(move || record_attempt(&db_path, &resource))
        .await
        .map_err(|error| anyhow!("refresh metadata worker failed: {error}"))?
}

async fn record_success_blocking(
    db_path: PathBuf,
    resource: ResourceKey,
    next_refresh_after: Option<Timestamp>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || record_success(&db_path, &resource, next_refresh_after))
        .await
        .map_err(|error| anyhow!("refresh metadata worker failed: {error}"))?
}

async fn save_sync_result_blocking(db_path: PathBuf, result: SyncResult) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let repository = SqliteRepository::new(db_path);
        repository.save_sync_result(result)
    })
    .await
    .map_err(|error| anyhow!("repository worker failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use std::{fs, process};

    use tokio::time::{Duration, timeout};

    use super::*;
    use crate::data::sqlite::{self, DbLocation};

    #[tokio::test]
    async fn coordinator_dedupes_duplicate_resources_in_request() {
        let db_path = temp_db_path("dedupe");
        sqlite::initialize_at(db_path.clone(), DbLocation::ProjectLocal).expect("db init");
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let coordinator =
            RefreshCoordinator::start(db_path.clone(), crate::config::WORLD_CUP_2026, event_sender);

        coordinator
            .request(RefreshRequest::manual(vec![
                ResourceKey::Stages,
                ResourceKey::Stages,
            ]))
            .expect("request");

        let mut saw_started = false;
        let mut saw_deduped = false;
        let mut saw_succeeded = false;

        for _ in 0..3 {
            let event = timeout(Duration::from_secs(2), event_receiver.recv())
                .await
                .expect("event timeout")
                .expect("event");
            match event {
                RefreshEvent::Started { resource } if resource == ResourceKey::Stages => {
                    saw_started = true;
                }
                RefreshEvent::Deduped { resource } if resource == ResourceKey::Stages => {
                    saw_deduped = true;
                }
                RefreshEvent::Succeeded { resource, .. } if resource == ResourceKey::Stages => {
                    saw_succeeded = true;
                }
                _ => {}
            }
        }

        assert!(saw_started);
        assert!(saw_deduped);
        assert!(saw_succeeded);

        let _ = fs::remove_file(db_path);
    }

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "footui-coordinator-{name}-{}.sqlite3",
            process::id()
        ))
    }
}
