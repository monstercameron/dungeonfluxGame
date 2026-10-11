#!/usr/bin/env python3
"""Execute the local devlog append contract against an isolated, actual-schema fixture."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_PATH = Path(__file__).parent / "fixtures" / "devlog_append_authority.json"
QUEUE_TABLES = ("features", "tasks", "dependencies", "attempts")
SCRATCH = None
DEADLINE = None
COMMANDS = []
CASE_NUMBER = 0


def source_pin(path):
    digest = hashlib.sha256()
    byte_count = 0
    with path.open("rb") as source:
        for block in iter(lambda: source.read(16384), b""):
            digest.update(block)
            byte_count += len(block)
    return {"path": str(path), "bytes": byte_count, "sha256": digest.hexdigest()}


class DevlogAppendAuthority(unittest.TestCase):
    def setUp(self):
        global CASE_NUMBER
        CASE_NUMBER += 1
        self.directory = SCRATCH / ("case-%02d" % CASE_NUMBER)
        self.directory.mkdir(mode=0o700)
        self.database = self.directory / "workflow.sqlite3"
        self.context_path = self.directory / "context.json"
        self.entry_path = self.directory / "entry.json"
        self.connection = sqlite3.connect(self.database)
        self.addCleanup(self.connection.close)
        self.connection.executescript((ROOT / "development" / "schema.sql").read_text())
        self.connection.execute("PRAGMA foreign_keys=ON")
        self.connection.execute(
            "INSERT INTO features VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            ("fixture-feature", "crosscutting", "Fixture", "Fixture", '["fixture"]',
             90, "active", "{}", "fixture", "2000-01-01", "2000-01-01"),
        )
        for task_id in ("fixture-task", "other-task"):
            self.connection.execute(
                "INSERT INTO tasks(id,feature_id,title,objective,stage,edit_areas_json,"
                "acceptance_json,verification_json,brief_json,status,priority,"
                "source_fingerprint,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                (task_id, "fixture-feature", "Fixture", "Fixture", "implementation",
                 '["fixture"]', '["fixture"]', '["fixture"]', "{}", "pending", 90,
                 "fixture", "2000-01-01", "2000-01-01"),
            )
        self.insert_attempt("fixture-attempt", "fixture-worker", 1, "fixture-token")
        self.connection.execute(
            "UPDATE tasks SET status='running',active_attempt_id=?,lease_generation=1 WHERE id=?",
            ("fixture-attempt", "fixture-task"),
        )
        self.connection.execute(
            "INSERT INTO devlog(id,created_at,agent_id,role,kind,summary,details,action,outcome)"
            " VALUES(?,?,?,?,?,?,?,?,?)",
            ("other-observation", "2000-01-01T00:00:00+00:00", "other-worker", "worker",
             "discovery", "Other fixture", "Synthetic fixture", "Synthetic action", "Retained"),
        )
        self.connection.commit()
        self.context = {
            "agent_id": "fixture-worker", "role": "worker",
            "task_id": "fixture-task", "attempt_id": "fixture-attempt",
        }
        self.entry = {
            "id": "worker-observation", "kind": "discovery", "summary": "Fixture discovery",
            "details": "Synthetic non-private observation.", "action": "Exercise actual local CLI.",
            "outcome": "Fixture observation retained.",
        }

    def insert_attempt(self, attempt_id, worker, generation, token):
        self.connection.execute(
            "INSERT INTO attempts(id,task_id,worker,model,role,status,phase,lease_owner,"
            "lease_token,lease_generation,brief_revision,brief_json,lease_expires_at,"
            "evidence_json,defects_json,started_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            (attempt_id, "fixture-task", worker, "fixture-model", "worker", "active",
             "implementation", worker, token, generation, "fixture", "{}",
             "2099-01-01T00:00:00+00:00", "[]", "[]", "2000-01-01T00:00:00+00:00"),
        )

    def queue_rows(self):
        return {
            table: self.connection.execute("SELECT * FROM " + table + " ORDER BY rowid").fetchall()
            for table in QUEUE_TABLES
        }

    def observations(self):
        return self.connection.execute("SELECT * FROM devlog ORDER BY id").fetchall()

    def invoke(self, entry=None, context=None, accepted=True):
        before_queue = self.queue_rows()
        before_observations = self.observations()
        self.context_path.write_text(json.dumps(self.context if context is None else context))
        self.entry_path.write_text(json.dumps(self.entry if entry is None else entry))
        remaining = DEADLINE - time.monotonic()
        self.assertGreater(remaining, 0, "Finite suite deadline exhausted before dispatch")
        command = [
            sys.executable, "-S", str(ROOT / "development" / "devlog.py"),
            "--database", str(self.database), "--context", str(self.context_path),
            "--entry", str(self.entry_path),
        ]
        result = subprocess.run(
            command, cwd=self.directory, capture_output=True, text=True,
            timeout=min(3, remaining),
            env={"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
                 "TZ": "UTC", "TMPDIR": str(self.directory)},
        )
        COMMANDS.append({
            "case": self._testMethodName, "command": command, "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr,
        })
        self.assertLessEqual(len(result.stdout.encode()) + len(result.stderr.encode()), 16384)
        self.assertEqual(self.queue_rows(), before_queue, "Devlog CLI changed authoritative queue rows")
        if accepted:
            self.assertEqual(result.returncode, 0, result.stderr)
            response = json.loads(result.stdout)
            self.assertEqual(response["queue_state_changed"], False)
            self.assertEqual(response["id"], (self.entry if entry is None else entry)["id"])
            return response
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.observations(), before_observations, "Refused append changed retained rows")
        return result.stderr

    def test_worker_appends_observation_with_context_owned_provenance(self):
        response = self.invoke()
        self.assertEqual(response["result"], "appended")
        row = self.connection.execute(
            "SELECT agent_id,role,task_id,attempt_id,created_at,details FROM devlog WHERE id=?",
            (self.entry["id"],),
        ).fetchone()
        self.assertEqual(row[:4], ("fixture-worker", "worker", "fixture-task", "fixture-attempt"))
        timestamp = datetime.datetime.fromisoformat(row[4])
        self.assertEqual(timestamp.utcoffset(), datetime.timedelta(0))
        self.assertEqual(row[5], self.entry["details"])

    def test_entry_cannot_supply_identity_role_task_attempt_or_timestamp(self):
        for field in ("agent_id", "role", "task_id", "attempt_id", "created_at"):
            with self.subTest(field=field):
                error = self.invoke(dict(self.entry, **{field: "forged"}), accepted=False)
                self.assertIn("identity/task references come from execution context", error)

    def test_unassigned_actor_and_mismatched_or_unknown_attempt_are_refused(self):
        contexts = (
            dict(self.context, agent_id="unassigned-worker"),
            dict(self.context, task_id="other-task"),
            dict(self.context, attempt_id="missing-attempt"),
        )
        for context in contexts:
            with self.subTest(context=context):
                self.invoke(context=context, accepted=False)

    def test_same_id_retry_is_idempotent_and_changed_retry_cannot_rewrite(self):
        self.invoke()
        original = self.observations()
        response = self.invoke()
        self.assertEqual(response["result"], "existing")
        self.assertEqual(self.observations(), original)
        self.invoke(dict(self.entry, details="Changed immutable observation"), accepted=False)
        self.assertEqual(self.observations(), original)

    def test_correction_is_a_new_linked_row(self):
        self.invoke()
        original = self.observations()
        correction = dict(
            self.entry, id="worker-correction", kind="resolution",
            details="Linked correction", related_entry_id=self.entry["id"],
        )
        self.invoke(correction)
        self.assertEqual(
            self.connection.execute("SELECT related_entry_id FROM devlog WHERE id=?",
                                    (correction["id"],)).fetchone(),
            (self.entry["id"],),
        )
        for row in original:
            self.assertIn(row, self.observations())

    def test_worker_cannot_update_or_delete_own_or_other_entries(self):
        self.invoke()
        original = self.observations()
        queue = self.queue_rows()
        for observation_id in (self.entry["id"], "other-observation"):
            for statement in (
                "UPDATE devlog SET details='rewritten' WHERE id=?",
                "DELETE FROM devlog WHERE id=?",
            ):
                with self.subTest(observation_id=observation_id, statement=statement):
                    with self.assertRaisesRegex(sqlite3.IntegrityError, "devlog is append only"):
                        self.connection.execute(statement, (observation_id,))
                    self.connection.rollback()
                    self.assertEqual(self.observations(), original)
                    self.assertEqual(self.queue_rows(), queue)

    def test_late_known_superseded_attempt_remains_historical(self):
        self.connection.execute(
            "UPDATE attempts SET status='abandoned',phase='terminal',finished_at=? WHERE id=?",
            ("2000-01-02T00:00:00+00:00", "fixture-attempt"),
        )
        self.insert_attempt("new-attempt", "new-worker", 2, "new-token")
        self.connection.execute(
            "UPDATE tasks SET active_attempt_id=?,lease_generation=2 WHERE id=?",
            ("new-attempt", "fixture-task"),
        )
        self.connection.commit()
        self.invoke()
        self.assertEqual(
            self.connection.execute("SELECT attempt_id FROM devlog WHERE id=?",
                                    (self.entry["id"],)).fetchone(),
            ("fixture-attempt",),
        )
        self.assertEqual(
            self.connection.execute("SELECT status,phase FROM attempts WHERE id=?",
                                    ("fixture-attempt",)).fetchone(),
            ("abandoned", "terminal"),
        )

    def test_unlinked_observation_uses_context_identity(self):
        context = {"agent_id": "fixture-worker", "role": "worker", "task_id": None, "attempt_id": None}
        self.invoke(context=context)
        self.assertEqual(
            self.connection.execute("SELECT agent_id,task_id,attempt_id FROM devlog WHERE id=?",
                                    (self.entry["id"],)).fetchone(),
            ("fixture-worker", None, None),
        )

    def test_invalid_kind_bounds_stable_id_and_dangling_correction_refuse(self):
        invalid_entries = (
            dict(self.entry, kind="status"),
            dict(self.entry, summary="x" * 301),
            dict(self.entry, id=" padded-id "),
            dict(self.entry, related_entry_id="missing-observation"),
        )
        for entry in invalid_entries:
            with self.subTest(entry=entry):
                self.invoke(entry, accepted=False)

    def test_local_context_file_is_not_trusted_runner_security(self):
        fixture = json.loads(FIXTURE_PATH.read_text())
        self.assertEqual(fixture["task_id"], "B-G09-D04")
        self.assertEqual(fixture["authority"], "local planning-phase role contract")
        self.assertEqual(fixture["production_trusted_runner_implemented"], False)
        self.assertEqual(fixture["anti_forgery_claim"], False)
        self.assertEqual(fixture["acceptance"][0], "worker append only")
        self.assertGreaterEqual(len(fixture["unresolved"]), 2)
        self.assertEqual(
            set(fixture["cases"]), {name.removeprefix("test_") for name in dir(self)
                                   if name.startswith("test_")},
        )


def main():
    global SCRATCH, DEADLINE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scratch", type=Path, required=True)
    args = parser.parse_args()
    if not args.scratch.is_absolute() or args.scratch.is_symlink():
        parser.error("--scratch must be an absolute, fresh owned artifact directory")
    scratch = args.scratch.resolve()
    allowed_parents = [ROOT / "artifacts"]
    if ROOT.parent.name == "worktrees" and ROOT.parent.parent.name == "artifacts":
        allowed_parents.append(ROOT.parents[2] / "artifacts")
    if not any(scratch.is_relative_to(parent) for parent in allowed_parents):
        parser.error("--scratch must stay in project-owned artifacts")
    scratch.mkdir(mode=0o700)
    os.chmod(scratch, 0o700)
    SCRATCH = scratch
    DEADLINE = time.monotonic() + 45
    sources = [
        Path(__file__).resolve(), FIXTURE_PATH, ROOT / "development" / "devlog.py",
        ROOT / "development" / "schema.sql", Path(sys.executable), Path('/usr/bin/python3'),
    ]
    before = [source_pin(path) for path in sources]
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(DevlogAppendAuthority)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    after = [source_pin(path) for path in sources]
    unchanged = before == after
    print(json.dumps({
        "task_id": "B-G09-D04", "tests_run": result.testsRun,
        "outcome": "PASS" if result.wasSuccessful() and unchanged else "FAIL",
        "source_inputs_before": before, "source_inputs_after": after,
        "source_inputs_unchanged": unchanged, "commands": COMMANDS,
        "rust_wasm_browser_checks": "UNPERFORMED_NOT_APPLICABLE_TO_PYTHON_TOOLING",
        "trusted_runner_access_binding": "UNIMPLEMENTED_FUTURE_GATE",
    }))
    return 0 if result.wasSuccessful() and unchanged else 1


if __name__ == "__main__":
    raise SystemExit(main())
