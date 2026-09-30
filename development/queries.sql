-- Open read-only: sqlite3 -readonly development/workflow.sqlite3
-- Plans grouped by kind/status; game work remains planned/pending.
SELECT kind,status,count(*) AS count FROM features GROUP BY kind,status ORDER BY kind,status;
SELECT stage,status,count(*) AS count FROM tasks GROUP BY stage,status ORDER BY stage,status;
-- Potential dependency-ready plans are NOT automatically dispatch-authorized atomic tasks.
SELECT * FROM dependency_ready_plans ORDER BY priority DESC,id LIMIT 50;
SELECT * FROM dispatch_ready_tasks ORDER BY priority DESC,id LIMIT 50;
-- Gate owners, decision inputs/output and unresolved prerequisites.
SELECT id,title,json_extract(plan_json,'$.owners') AS owners,json_extract(plan_json,'$.models') AS inputs FROM features WHERE kind='gate' ORDER BY id;
-- Retrieve a brief with its exact input fingerprints and direct prerequisites.
SELECT id,objective,brief_json FROM tasks WHERE id='G02-RESOLVE';
SELECT task_id,prerequisite_task_id FROM dependencies WHERE task_id='G02-RESOLVE';
-- Public contract and model ledger for a subsystem.
SELECT id,json_extract(plan_json,'$.models') AS models,json_extract(plan_json,'$.public_boundary') AS boundary,json_extract(plan_json,'$.direct_dependencies') AS edges FROM features WHERE id='C-df-session';
-- Append-only findings and linked resolutions, newest first.
SELECT id,created_at,kind,summary,related_entry_id,evidence_ref FROM devlog ORDER BY created_at DESC,id LIMIT 50;
SELECT id,task_id,role,status,verdict,evidence_json,defects_json FROM attempts ORDER BY started_at DESC,id LIMIT 20;
-- Data consistency checks, expected 'ok' and no rows respectively.
PRAGMA integrity_check;
PRAGMA foreign_key_check;
