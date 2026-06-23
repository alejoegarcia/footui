pragma foreign_keys = on;

create table cache_entries(
  key text primary key,
  url text not null,
  fetched_at text not null,
  expires_at text not null,
  body_hash text,
  raw_jsonb blob not null check (typeof(raw_jsonb) = 'blob' and json_valid(raw_jsonb, 4))
) strict, without rowid;

create table teams(
  id text primary key,
  name text not null,
  abbreviation text not null check (length(abbreviation) between 2 and 4),
  country_code text not null check (length(country_code) = 3),
  confederation text not null check (confederation in ('AFC', 'CAF', 'CONCACAF', 'CONMEBOL', 'OFC', 'UEFA')),
  flag_url_template text,
  updated_at text not null
) strict, without rowid;

create table favorite_teams(
  team_id text primary key,
  created_at text not null,
  foreign key(team_id) references teams(id)
) strict, without rowid;

create table stages(
  id text primary key,
  name text not null,
  stage_type integer not null check (stage_type in (0, 1)),
  sort_order integer not null check (sort_order between 1 and 7),
  check (id in ('289273', '289287', '289288', '289289', '289290', '289291', '289292'))
) strict, without rowid;

create table groups(
  id text primary key,
  name text not null check (name in ('Group A', 'Group B', 'Group C', 'Group D', 'Group E', 'Group F', 'Group G', 'Group H', 'Group I', 'Group J', 'Group K', 'Group L')),
  stage_id text not null check (stage_id = '289273'),
  foreign key(stage_id) references stages(id),
  check (id in ('289275', '289276', '289277', '289278', '289279', '289280', '289281', '289282', '289283', '289284', '289285', '289286'))
) strict, without rowid;

create table matches(
  id text primary key,
  match_number integer not null check (match_number between 1 and 104),
  stage_id text not null,
  group_id text,
  utc_start text not null,
  local_start text,
  home_team_id text,
  away_team_id text,
  home_team_name text not null,
  away_team_name text not null,
  home_score integer check (home_score is null or home_score >= 0),
  away_score integer check (away_score is null or away_score >= 0),
  home_penalty_score integer check (home_penalty_score is null or home_penalty_score >= 0),
  away_penalty_score integer check (away_penalty_score is null or away_penalty_score >= 0),
  status integer not null check (status >= 0),
  minute text,
  stadium_name text,
  attendance integer check (attendance is null or attendance >= 0),
  winner_team_id text,
  raw_jsonb blob not null check (typeof(raw_jsonb) = 'blob' and json_valid(raw_jsonb, 4)),
  updated_at text not null,
  foreign key(stage_id) references stages(id),
  foreign key(group_id) references groups(id),
  foreign key(home_team_id) references teams(id),
  foreign key(away_team_id) references teams(id),
  foreign key(winner_team_id) references teams(id)
) strict, without rowid;

create table standings(
  season_id text not null,
  stage_id text not null,
  group_id text not null,
  team_id text not null,
  position integer not null check (position between 1 and 4),
  played integer not null check (played between 0 and 3),
  won integer not null check (won between 0 and 3),
  drawn integer not null check (drawn between 0 and 3),
  lost integer not null check (lost between 0 and 3),
  goals_for integer not null check (goals_for >= 0),
  goals_against integer not null check (goals_against >= 0),
  goal_difference integer not null,
  points integer not null check (points between 0 and 9),
  qualification_status text,
  raw_jsonb blob not null check (typeof(raw_jsonb) = 'blob' and json_valid(raw_jsonb, 4)),
  updated_at text not null,
  foreign key(stage_id) references stages(id),
  foreign key(group_id) references groups(id),
  foreign key(team_id) references teams(id),
  primary key(season_id, stage_id, group_id, team_id)
) strict, without rowid;

create table player_stats(
  player_id text not null,
  season_id text not null,
  player_name text not null,
  team_id text not null,
  country_code text not null,
  rank integer check (rank is null or rank >= 1),
  matches_played integer check (matches_played is null or matches_played >= 0),
  minutes_played integer check (minutes_played is null or minutes_played >= 0),
  goals integer check (goals is null or goals >= 0),
  assists integer check (assists is null or assists >= 0),
  attempts integer check (attempts is null or attempts >= 0),
  attempts_on_target integer check (attempts_on_target is null or attempts_on_target >= 0),
  raw_jsonb blob not null check (typeof(raw_jsonb) = 'blob' and json_valid(raw_jsonb, 4)),
  updated_at text not null,
  foreign key(team_id) references teams(id),
  primary key(player_id, season_id)
) strict, without rowid;

create table timeline_events(
  match_id text not null,
  event_index integer not null,
  event_type integer,
  team_id text,
  player_id text,
  minute text,
  description text,
  raw_jsonb blob not null check (typeof(raw_jsonb) = 'blob' and json_valid(raw_jsonb, 4)),
  foreign key(match_id) references matches(id),
  foreign key(team_id) references teams(id),
  primary key(match_id, event_index)
) strict, without rowid;
