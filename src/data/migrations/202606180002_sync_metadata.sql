create table sync_runs(
  id integer primary key,
  requested_at text not null,
  reason text not null check (reason in ('startup', 'manual', 'screen_enter', 'policy')),
  resource_count integer not null check (resource_count >= 0),
  status text not null check (status in ('queued', 'running', 'complete', 'partial', 'failed')),
  completed_at text
) strict;

create table resource_refreshes(
  resource_key text primary key,
  resource_type text not null,
  last_attempt_at text,
  last_success_at text,
  next_refresh_after text,
  last_error text,
  updated_at text not null
) strict, without rowid;
