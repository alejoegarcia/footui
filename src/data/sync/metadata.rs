use std::path::Path;

use anyhow::{Context, Result};
use jiff::Timestamp;
use rusqlite::{Connection, params};

use crate::data::{sqlite, sync::ResourceKey};

pub fn record_attempt(db_path: &Path, resource: &ResourceKey) -> Result<()> {
    let connection = open(db_path)?;
    connection
        .execute(
            "
            insert into resource_refreshes(
              resource_key,
              resource_type,
              last_attempt_at,
              updated_at
            )
            values(?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            on conflict(resource_key) do update set
              resource_type = excluded.resource_type,
              last_attempt_at = excluded.last_attempt_at,
              updated_at = excluded.updated_at
            ",
            params![resource.storage_key(), resource.resource_type()],
        )
        .with_context(|| format!("recording refresh attempt for {resource}"))?;

    Ok(())
}

pub fn record_success(
    db_path: &Path,
    resource: &ResourceKey,
    next_refresh_after: Option<Timestamp>,
) -> Result<()> {
    let connection = open(db_path)?;
    let next_refresh_after = next_refresh_after.map(|timestamp| timestamp.to_string());
    connection
        .execute(
            "
            insert into resource_refreshes(
              resource_key,
              resource_type,
              last_success_at,
              next_refresh_after,
              last_error,
              updated_at
            )
            values(?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3, null, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            on conflict(resource_key) do update set
              resource_type = excluded.resource_type,
              last_success_at = excluded.last_success_at,
              next_refresh_after = excluded.next_refresh_after,
              last_error = null,
              updated_at = excluded.updated_at
            ",
            params![
                resource.storage_key(),
                resource.resource_type(),
                next_refresh_after
            ],
        )
        .with_context(|| format!("recording refresh success for {resource}"))?;

    Ok(())
}

pub fn record_error(db_path: &Path, resource: &ResourceKey, error: &str) -> Result<()> {
    let connection = open(db_path)?;
    connection
        .execute(
            "
            insert into resource_refreshes(
              resource_key,
              resource_type,
              last_error,
              updated_at
            )
            values(?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            on conflict(resource_key) do update set
              resource_type = excluded.resource_type,
              last_error = excluded.last_error,
              updated_at = excluded.updated_at
            ",
            params![resource.storage_key(), resource.resource_type(), error],
        )
        .with_context(|| format!("recording refresh error for {resource}"))?;

    Ok(())
}

fn open(db_path: &Path) -> Result<Connection> {
    let connection = Connection::open(db_path)
        .with_context(|| format!("opening sync metadata database {}", db_path.display()))?;
    sqlite::configure_connection(&connection)?;
    Ok(connection)
}

#[cfg(test)]
mod tests {
    use std::{fs, process};

    use super::*;
    use crate::data::sqlite::{self, DbLocation};

    #[test]
    fn records_refresh_success_metadata() {
        let db_path = temp_db_path("metadata-success");
        sqlite::initialize_at(db_path.clone(), DbLocation::ProjectLocal).expect("db init");

        let resource = ResourceKey::Teams;
        record_attempt(&db_path, &resource).expect("attempt");
        record_success(&db_path, &resource, None).expect("success");

        let connection = Connection::open(&db_path).expect("open");
        let last_success_at: String = connection
            .query_row(
                "select last_success_at from resource_refreshes where resource_key = 'teams'",
                [],
                |row| row.get(0),
            )
            .expect("last_success_at");
        assert!(!last_success_at.is_empty());

        let _ = fs::remove_file(db_path);
    }

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("footui-{name}-{}.sqlite3", process::id()))
    }
}
