alter table teams add column fifa_rank integer check (fifa_rank is null or fifa_rank >= 1);
alter table teams add column fifa_ranking_points real check (fifa_ranking_points is null or fifa_ranking_points >= 0);
