PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS features (
 id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('capability','rules','crate','gate','slice','crosscutting','planning')),
 title TEXT NOT NULL CHECK(length(title)>0), goal TEXT NOT NULL CHECK(length(goal)>0),
 acceptance_json TEXT NOT NULL CHECK(json_valid(acceptance_json) AND json_array_length(acceptance_json)>0),
 priority INTEGER NOT NULL CHECK(priority BETWEEN 0 AND 100), status TEXT NOT NULL CHECK(status IN ('planned','active','review','done','blocked','cancelled')),
 plan_json TEXT NOT NULL CHECK(json_valid(plan_json)), source_fingerprint TEXT NOT NULL,
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS tasks (
 id TEXT PRIMARY KEY NOT NULL, feature_id TEXT NOT NULL REFERENCES features(id), title TEXT NOT NULL,
 objective TEXT NOT NULL, stage TEXT NOT NULL CHECK(stage IN ('resolution','design','implementation','acceptance','optimization','planning')),
 edit_areas_json TEXT NOT NULL CHECK(json_valid(edit_areas_json) AND json_array_length(edit_areas_json)>0),
 acceptance_json TEXT NOT NULL CHECK(json_valid(acceptance_json) AND json_array_length(acceptance_json)>0),
 verification_json TEXT NOT NULL CHECK(json_valid(verification_json) AND json_array_length(verification_json)>0),
 brief_json TEXT NOT NULL CHECK(json_valid(brief_json)), status TEXT NOT NULL CHECK(status IN ('pending','running','review','done','blocked','cancelled')),
 blocking_reason TEXT, originating_task_id TEXT REFERENCES tasks(id), priority INTEGER NOT NULL CHECK(priority BETWEEN 0 AND 100),
 source_fingerprint TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 lease_generation INTEGER NOT NULL DEFAULT 0 CHECK(lease_generation>=0), active_attempt_id TEXT REFERENCES attempts(id),
 CHECK(status!='blocked' OR (blocking_reason IS NOT NULL AND length(trim(blocking_reason))>0))
);
CREATE TABLE IF NOT EXISTS dependencies (
 task_id TEXT NOT NULL REFERENCES tasks(id), prerequisite_task_id TEXT NOT NULL REFERENCES tasks(id),
 PRIMARY KEY(task_id, prerequisite_task_id), CHECK(task_id!=prerequisite_task_id)
);
CREATE TABLE IF NOT EXISTS attempts (
 id TEXT PRIMARY KEY NOT NULL, task_id TEXT NOT NULL REFERENCES tasks(id), worker TEXT NOT NULL, model TEXT NOT NULL, effort TEXT,
 role TEXT NOT NULL CHECK(role IN ('worker','evaluator','coordinator','cleanup','critic')),
 status TEXT NOT NULL CHECK(status IN ('active','submitted','approved','rejected','inconclusive','integrated','abandoned')),
 phase TEXT NOT NULL DEFAULT 'implementation' CHECK(phase IN ('implementation','review','integration','terminal')),
 lease_owner TEXT, lease_token TEXT UNIQUE, lease_generation INTEGER NOT NULL DEFAULT 0 CHECK(lease_generation>=0),
 brief_revision TEXT NOT NULL, brief_json TEXT NOT NULL CHECK(json_valid(brief_json)), lease_expires_at TEXT,
 submitted_commit TEXT, integrated_commit TEXT, source_revision TEXT, tested_revision TEXT, integrated_revision TEXT,
 evaluator_id TEXT, evaluator_model TEXT, capabilities_json TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(capabilities_json)),
 review_evidence_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(review_evidence_json)),
 integration_evidence_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(integration_evidence_json)),
 evidence_json TEXT NOT NULL CHECK(json_valid(evidence_json)), verdict TEXT CHECK(verdict IN ('approve','reject','inconclusive')),
 defects_json TEXT NOT NULL CHECK(json_valid(defects_json)), started_at TEXT NOT NULL, finished_at TEXT,
 unsuccessful_implementation INTEGER NOT NULL DEFAULT 0 CHECK(unsuccessful_implementation IN (0,1)),
 CHECK(verdict IS NULL OR evaluator_id IS NOT NULL)
);
CREATE TABLE IF NOT EXISTS devlog (
 id TEXT PRIMARY KEY NOT NULL, created_at TEXT NOT NULL, agent_id TEXT NOT NULL, role TEXT NOT NULL,
 task_id TEXT REFERENCES tasks(id), attempt_id TEXT REFERENCES attempts(id),
 kind TEXT NOT NULL CHECK(kind IN ('confusion','error','defect','blocker','challenge','discovery','resolution')),
 summary TEXT NOT NULL, details TEXT NOT NULL, action TEXT NOT NULL, outcome TEXT NOT NULL,
 suggestion TEXT, related_entry_id TEXT REFERENCES devlog(id), evidence_ref TEXT,
 CHECK(attempt_id IS NULL OR task_id IS NOT NULL)
);
CREATE INDEX IF NOT EXISTS tasks_selection ON tasks(status,priority,feature_id);
CREATE INDEX IF NOT EXISTS dependency_reverse ON dependencies(prerequisite_task_id,task_id);
CREATE INDEX IF NOT EXISTS attempts_task ON attempts(task_id,started_at);
CREATE INDEX IF NOT EXISTS attempts_lease ON attempts(status,lease_expires_at);
CREATE INDEX IF NOT EXISTS devlog_task ON devlog(task_id,created_at);
CREATE INDEX IF NOT EXISTS devlog_attempt ON devlog(attempt_id,created_at);
CREATE INDEX IF NOT EXISTS devlog_kind ON devlog(kind,created_at);
CREATE TRIGGER IF NOT EXISTS dependencies_no_cycle BEFORE INSERT ON dependencies
BEGIN
 SELECT CASE WHEN EXISTS(
  WITH RECURSIVE reach(id) AS (
   SELECT NEW.prerequisite_task_id UNION SELECT d.prerequisite_task_id FROM dependencies d JOIN reach r ON d.task_id=r.id
  ) SELECT 1 FROM reach WHERE id=NEW.task_id
 ) THEN RAISE(ABORT,'dependency cycle') END;
END;
CREATE TRIGGER IF NOT EXISTS devlog_no_update BEFORE UPDATE ON devlog BEGIN SELECT RAISE(ABORT,'devlog is append only'); END;
CREATE TRIGGER IF NOT EXISTS devlog_no_delete BEFORE DELETE ON devlog BEGIN SELECT RAISE(ABORT,'devlog is append only'); END;
CREATE TRIGGER IF NOT EXISTS devlog_attempt_matches BEFORE INSERT ON devlog
WHEN NEW.attempt_id IS NOT NULL AND (SELECT task_id FROM attempts WHERE id=NEW.attempt_id)!=NEW.task_id
BEGIN SELECT RAISE(ABORT,'devlog task/attempt mismatch'); END;
PRAGMA user_version = 2;
CREATE TRIGGER IF NOT EXISTS dependencies_no_update BEFORE UPDATE ON dependencies BEGIN SELECT RAISE(ABORT,'replace dependency by validated delete/insert'); END;
CREATE VIEW IF NOT EXISTS dependency_ready_plans AS
SELECT t.id,t.feature_id,t.title,t.stage,t.priority FROM tasks t
WHERE t.status='pending' AND NOT EXISTS(
 SELECT 1 FROM dependencies d JOIN tasks p ON p.id=d.prerequisite_task_id WHERE d.task_id=t.id AND p.status!='done'
);
CREATE VIEW IF NOT EXISTS dispatch_ready_tasks AS
SELECT t.id,t.feature_id,t.title,t.stage,t.priority FROM tasks t
WHERE t.status='pending' AND json_extract(t.brief_json,'$.dispatch_ready')=1 AND NOT EXISTS(
 SELECT 1 FROM dependencies d JOIN tasks p ON p.id=d.prerequisite_task_id WHERE d.task_id=t.id AND p.status!='done'
);

DROP INDEX IF EXISTS one_live_attempt;
CREATE UNIQUE INDEX one_live_attempt ON attempts(task_id) WHERE status IN ('active','submitted','approved','inconclusive');
DROP TRIGGER IF EXISTS attempt_independent_approval;
CREATE TRIGGER attempt_independent_approval BEFORE UPDATE ON attempts
WHEN NEW.verdict='approve'
BEGIN
 SELECT CASE WHEN NEW.evaluator_id IS NULL OR length(trim(NEW.evaluator_id))=0 OR NEW.evaluator_id=NEW.worker OR NEW.evaluator_model IS NULL OR length(trim(NEW.evaluator_model))=0
  OR NEW.tested_revision IS NULL OR length(trim(NEW.tested_revision))=0 OR json_array_length(NEW.review_evidence_json)=0
  OR coalesce(json_extract(NEW.capabilities_json,'$.frontier'),0)!=1
  OR EXISTS(SELECT 1 FROM json_each((SELECT acceptance_json FROM tasks WHERE id=NEW.task_id)) required
   WHERE NOT EXISTS(SELECT 1 FROM json_each(NEW.review_evidence_json) observed
    WHERE CASE WHEN observed.type='object' THEN json_extract(observed.value,'$.criterion') END=required.value
     AND CASE WHEN observed.type='object' THEN json_extract(observed.value,'$.result') END='pass'
     AND length(coalesce(CASE WHEN observed.type='object' THEN json_extract(observed.value,'$.evidence_ref') END,''))>0))
  OR (coalesce(json_extract((SELECT brief_json FROM tasks WHERE id=NEW.task_id),'$.required_capabilities.computer_use'),0)=1 AND coalesce(json_extract(NEW.capabilities_json,'$.computer_use_enabled'),0)!=1)
  OR (coalesce(json_extract((SELECT brief_json FROM tasks WHERE id=NEW.task_id),'$.required_capabilities.vision'),0)=1 AND coalesce(json_extract(NEW.capabilities_json,'$.vision_enabled'),0)!=1)
  OR (coalesce(json_extract((SELECT brief_json FROM tasks WHERE id=NEW.task_id),'$.required_capabilities.audio'),0)=1 AND coalesce(json_extract(NEW.capabilities_json,'$.audio_observation'),0)!=1)
 THEN RAISE(ABORT,'approval needs independent frontier criterion evidence and tested identity') END;
END;
DROP TRIGGER IF EXISTS attempt_independent_approval_insert;
CREATE TRIGGER attempt_independent_approval_insert BEFORE INSERT ON attempts
WHEN NEW.verdict='approve'
BEGIN
 SELECT CASE WHEN NEW.evaluator_id IS NULL OR length(trim(NEW.evaluator_id))=0 OR NEW.evaluator_id=NEW.worker OR NEW.evaluator_model IS NULL OR length(trim(NEW.evaluator_model))=0
  OR NEW.tested_revision IS NULL OR length(trim(NEW.tested_revision))=0 OR json_array_length(NEW.review_evidence_json)=0
  OR coalesce(json_extract(NEW.capabilities_json,'$.frontier'),0)!=1
  OR EXISTS(SELECT 1 FROM json_each((SELECT acceptance_json FROM tasks WHERE id=NEW.task_id)) required
   WHERE NOT EXISTS(SELECT 1 FROM json_each(NEW.review_evidence_json) observed
    WHERE CASE WHEN observed.type='object' THEN json_extract(observed.value,'$.criterion') END=required.value
     AND CASE WHEN observed.type='object' THEN json_extract(observed.value,'$.result') END='pass'
     AND length(coalesce(CASE WHEN observed.type='object' THEN json_extract(observed.value,'$.evidence_ref') END,''))>0))
  OR (coalesce(json_extract((SELECT brief_json FROM tasks WHERE id=NEW.task_id),'$.required_capabilities.computer_use'),0)=1 AND coalesce(json_extract(NEW.capabilities_json,'$.computer_use_enabled'),0)!=1)
  OR (coalesce(json_extract((SELECT brief_json FROM tasks WHERE id=NEW.task_id),'$.required_capabilities.vision'),0)=1 AND coalesce(json_extract(NEW.capabilities_json,'$.vision_enabled'),0)!=1)
  OR (coalesce(json_extract((SELECT brief_json FROM tasks WHERE id=NEW.task_id),'$.required_capabilities.audio'),0)=1 AND coalesce(json_extract(NEW.capabilities_json,'$.audio_observation'),0)!=1)
 THEN RAISE(ABORT,'approval needs independent frontier criterion evidence and tested identity') END;
END;
DROP TRIGGER IF EXISTS attempt_fenced_submission;
CREATE TRIGGER attempt_fenced_submission BEFORE UPDATE OF status,phase ON attempts
WHEN NEW.status IN ('submitted','approved','integrated') AND NEW.role NOT IN ('critic','evaluator')
BEGIN
 SELECT CASE WHEN (SELECT active_attempt_id FROM tasks WHERE id=NEW.task_id) IS NOT NEW.id
  OR (SELECT lease_generation FROM tasks WHERE id=NEW.task_id) IS NOT NEW.lease_generation
  OR length(coalesce(trim(NEW.lease_token),''))=0 OR length(coalesce(trim(NEW.lease_owner),''))=0
  OR julianday(NEW.lease_expires_at) IS NULL OR julianday(NEW.lease_expires_at)<=julianday('now')
 THEN RAISE(ABORT,'superseded or expired attempt cannot submit/integrate') END;
END;
DROP TRIGGER IF EXISTS task_done_requires_evidence;
CREATE TRIGGER task_done_requires_evidence BEFORE UPDATE OF status ON tasks
WHEN NEW.status='done' AND OLD.status!='done'
BEGIN
 SELECT CASE WHEN OLD.status!='review' OR EXISTS(SELECT 1 FROM dependencies d JOIN tasks p ON p.id=d.prerequisite_task_id WHERE d.task_id=NEW.id AND p.status!='done')
  OR NOT EXISTS(SELECT 1 FROM attempts a WHERE a.task_id=NEW.id AND a.status='integrated' AND a.verdict='approve'
    AND a.evaluator_id IS NOT NULL AND a.evaluator_id!=a.worker AND a.tested_revision IS NOT NULL
    AND a.role IN ('worker','coordinator','cleanup') AND a.lease_generation=NEW.lease_generation
    AND a.id=NEW.active_attempt_id AND a.lease_token IS NOT NULL AND a.integrated_revision=a.tested_revision AND json_array_length(a.review_evidence_json)>0
    AND json_array_length(a.integration_evidence_json)>0 AND coalesce(json_extract(a.capabilities_json,'$.frontier'),0)=1)
 THEN RAISE(ABORT,'done requires completed prerequisites and independent tested/integrated evidence') END;
END;
CREATE TRIGGER IF NOT EXISTS task_no_done_insert BEFORE INSERT ON tasks WHEN NEW.status='done'
BEGIN SELECT RAISE(ABORT,'create pending task then complete through reviewed integration'); END;

CREATE TRIGGER IF NOT EXISTS feature_done_requires_tasks BEFORE UPDATE OF status ON features
WHEN NEW.status='done' AND OLD.status!='done'
BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM tasks WHERE feature_id=NEW.id)
  OR EXISTS(SELECT 1 FROM tasks WHERE feature_id=NEW.id AND status!='done')
 THEN RAISE(ABORT,'feature done requires all scoped tasks done; rescope conditional work explicitly') END;
END;
CREATE TRIGGER IF NOT EXISTS feature_no_done_insert BEFORE INSERT ON features WHEN NEW.status='done'
BEGIN SELECT RAISE(ABORT,'create planned feature before verified completion'); END;

CREATE TRIGGER IF NOT EXISTS attempt_insert_starts_active BEFORE INSERT ON attempts
WHEN NEW.role IN ('worker','coordinator','cleanup') AND (NEW.status!='active' OR NEW.phase!='implementation')
BEGIN SELECT RAISE(ABORT,'implementation attempt must begin active and follow fenced phases'); END;
CREATE TRIGGER IF NOT EXISTS attempt_identity_immutable BEFORE UPDATE ON attempts
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.worker IS NOT OLD.worker OR NEW.model IS NOT OLD.model
 OR NEW.role IS NOT OLD.role OR NEW.brief_revision IS NOT OLD.brief_revision OR NEW.brief_json IS NOT OLD.brief_json
BEGIN SELECT RAISE(ABORT,'attempt identity and preserved brief are immutable'); END;
CREATE TRIGGER IF NOT EXISTS task_active_attempt_matches BEFORE UPDATE OF active_attempt_id,lease_generation ON tasks
WHEN NEW.active_attempt_id IS NOT NULL
BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM attempts a WHERE a.id=NEW.active_attempt_id AND a.task_id=NEW.id AND a.lease_generation=NEW.lease_generation)
 THEN RAISE(ABORT,'active attempt must belong to task and current lease generation') END;
END;
CREATE TRIGGER IF NOT EXISTS task_initial_active_attempt_matches BEFORE INSERT ON tasks
WHEN NEW.active_attempt_id IS NOT NULL
BEGIN SELECT RAISE(ABORT,'create unclaimed task before attempt assignment'); END;

CREATE TRIGGER IF NOT EXISTS features_stable_id_insert BEFORE INSERT ON features
WHEN NEW.id IS NULL OR length(trim(NEW.id))=0 OR NEW.id!=trim(NEW.id)
BEGIN SELECT RAISE(ABORT,'stable nonempty ID required'); END;
CREATE TRIGGER IF NOT EXISTS features_stable_id_update BEFORE UPDATE OF id ON features
WHEN NEW.id IS NOT OLD.id
BEGIN SELECT RAISE(ABORT,'stable IDs are immutable'); END;

CREATE TRIGGER IF NOT EXISTS tasks_stable_id_insert BEFORE INSERT ON tasks
WHEN NEW.id IS NULL OR length(trim(NEW.id))=0 OR NEW.id!=trim(NEW.id)
BEGIN SELECT RAISE(ABORT,'stable nonempty ID required'); END;
CREATE TRIGGER IF NOT EXISTS tasks_stable_id_update BEFORE UPDATE OF id ON tasks
WHEN NEW.id IS NOT OLD.id
BEGIN SELECT RAISE(ABORT,'stable IDs are immutable'); END;

CREATE TRIGGER IF NOT EXISTS attempts_stable_id_insert BEFORE INSERT ON attempts
WHEN NEW.id IS NULL OR length(trim(NEW.id))=0 OR NEW.id!=trim(NEW.id)
BEGIN SELECT RAISE(ABORT,'stable nonempty ID required'); END;
CREATE TRIGGER IF NOT EXISTS attempts_stable_id_update BEFORE UPDATE OF id ON attempts
WHEN NEW.id IS NOT OLD.id
BEGIN SELECT RAISE(ABORT,'stable IDs are immutable'); END;

CREATE TRIGGER IF NOT EXISTS devlog_stable_id_insert BEFORE INSERT ON devlog
WHEN NEW.id IS NULL OR length(trim(NEW.id))=0 OR NEW.id!=trim(NEW.id)
BEGIN SELECT RAISE(ABORT,'stable nonempty ID required'); END;
CREATE TRIGGER IF NOT EXISTS devlog_stable_id_update BEFORE UPDATE OF id ON devlog
WHEN NEW.id IS NOT OLD.id
BEGIN SELECT RAISE(ABORT,'stable IDs are immutable'); END;

CREATE TRIGGER IF NOT EXISTS dependency_stable_ids BEFORE INSERT ON dependencies
WHEN length(trim(NEW.task_id))=0 OR length(trim(NEW.prerequisite_task_id))=0
BEGIN SELECT RAISE(ABORT,'nonempty dependency IDs required'); END;

DROP TRIGGER IF EXISTS verdict_current_phase_fence;
CREATE TRIGGER verdict_current_phase_fence BEFORE UPDATE OF verdict ON attempts
WHEN NEW.role IN ('worker','coordinator','cleanup') AND NEW.verdict IS NOT NULL AND NEW.verdict IS NOT OLD.verdict
BEGIN
 SELECT CASE WHEN (SELECT active_attempt_id FROM tasks WHERE id=NEW.task_id) IS NOT NEW.id
 OR (SELECT lease_generation FROM tasks WHERE id=NEW.task_id) IS NOT NEW.lease_generation
 OR length(coalesce(trim(NEW.lease_token),''))=0 OR length(coalesce(trim(NEW.lease_owner),''))=0
 OR julianday(NEW.lease_expires_at) IS NULL OR julianday(NEW.lease_expires_at)<=julianday('now')
 OR NEW.phase!='review'
 THEN RAISE(ABORT,'review verdict requires current unexpired tracked phase ownership') END;
END;

-- Phase changes are atomic with status; uncertainty stays in review until retried or abandoned.
CREATE TRIGGER IF NOT EXISTS attempt_lifecycle_insert BEFORE INSERT ON attempts
WHEN NEW.role IN ('worker','coordinator','cleanup')
BEGIN
 SELECT CASE WHEN length(coalesce(trim(NEW.lease_owner),''))=0
 OR length(coalesce(trim(NEW.lease_token),''))=0 OR julianday(NEW.lease_expires_at) IS NULL
 OR julianday(NEW.lease_expires_at)<=julianday('now') OR NEW.lease_generation<1
 THEN RAISE(ABORT,'new implementation lease needs owner token generation and valid future expiry') END;
END;
CREATE TRIGGER IF NOT EXISTS attempt_lifecycle_update BEFORE UPDATE ON attempts
WHEN NEW.role IN ('worker','coordinator','cleanup')
BEGIN
 SELECT CASE WHEN length(coalesce(trim(NEW.lease_owner),''))=0
 OR length(coalesce(trim(NEW.lease_token),''))=0 OR julianday(NEW.lease_expires_at) IS NULL
 OR NEW.lease_generation<1
 OR NOT ((NEW.status='active' AND NEW.phase='implementation')
  OR (NEW.status IN ('submitted','inconclusive') AND NEW.phase='review')
  OR (NEW.status='approved' AND NEW.phase IN ('review','integration'))
  OR (NEW.status IN ('rejected','integrated','abandoned') AND NEW.phase='terminal'))
 THEN RAISE(ABORT,'invalid lease or status/phase combination') END;
 SELECT CASE WHEN NEW.status IS NOT OLD.status AND NOT (
  (OLD.status='active' AND NEW.status='submitted')
  OR (OLD.status IN ('submitted','inconclusive') AND NEW.status IN ('approved','rejected','inconclusive'))
  OR (OLD.status='approved' AND OLD.phase='integration' AND NEW.status='integrated')
  OR (OLD.status IN ('active','submitted','approved','inconclusive') AND NEW.status='abandoned'))
 THEN RAISE(ABORT,'attempt must follow submit review integration lifecycle') END;
 SELECT CASE WHEN NEW.status='approved' AND NEW.phase='integration'
  AND NOT (OLD.status='approved' AND OLD.phase IN ('review','integration'))
 THEN RAISE(ABORT,'integration starts after independent approval') END;
 SELECT CASE WHEN OLD.phase='integration' AND NEW.phase='review'
  OR (OLD.phase='terminal' AND NEW.phase!='terminal')
 THEN RAISE(ABORT,'attempt phase cannot move backward') END;
END;
CREATE TRIGGER IF NOT EXISTS task_reviewed_contract_immutable BEFORE UPDATE ON tasks
WHEN EXISTS(SELECT 1 FROM attempts a WHERE a.task_id=OLD.id
 AND a.role IN ('worker','coordinator','cleanup') AND a.status IN ('active','submitted','approved','inconclusive','integrated'))
 AND (NEW.acceptance_json IS NOT OLD.acceptance_json OR NEW.verification_json IS NOT OLD.verification_json
 OR NEW.brief_json IS NOT OLD.brief_json OR NEW.objective IS NOT OLD.objective
 OR NEW.edit_areas_json IS NOT OLD.edit_areas_json OR NEW.source_fingerprint IS NOT OLD.source_fingerprint
 OR NEW.stage IS NOT OLD.stage OR NEW.feature_id IS NOT OLD.feature_id)
BEGIN SELECT RAISE(ABORT,'claimed or integrated task contract is immutable; abandon or create scoped follow-up'); END;
