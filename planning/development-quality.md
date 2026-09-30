# Continuous development quality review

Date: 2026-09-29
Status: Personal skill/cache and one-minute heartbeat configured; Rust coordinator pending

The installed `dungeonflux-quality-review` skill selects bounded new/changed
authored Rust files, reviews code smells, complexity, duplication, bugs, test
evidence and Rust idioms, and retains findings. Its MD5 source/context/review-date
cache is `development/quality.sqlite3`, separate from the five-table
`development/workflow.sqlite3`. MD5 is change detection, not a security guarantee.
The skill resides at the configured runner's personal skill root; a fresh runner
must install/verify the skill and its actual capability before depending on it.

The configured thread heartbeat runs every minute and stays quiet without an
actionable change. No game source currently exists, so no source review has been
invented. Review helpers are development tools, not Python application code or
the planned Rust game/coordinator.

The helper claims at most three changed files for a bounded lease. Successful
reviews are skipped until source, policy or relevant context changes. Source or
context changes invalidate stale completion; failed/inconclusive checks do not
stamp a success and back off after two attempts on the same bytes/context.
Findings are durable; scratch reports are promoted into the quality database.
The cache and its SQLite sidecars never qualify for build-artifact cleanup.

Review agents cannot approve implementation or mutate queue state. The trusted
coordinator invokes the installed `scripts/quality_cache.py quality-intake`
command with project, finding ID and its identity to link a finding idempotently
into the workflow database and append devlog evidence. An unfinished original
acceptance defect stays on its task. Distinct work becomes a scoped pending task;
recurrence after completed work needs a revision-specific task and coordinator
assessment. New tasks are not dispatch-ready until their actual brief is frozen.
The role contract is not a filesystem security boundary.

Before creating a new proposal, intake rechecks that the authored file is still
eligible, its MD5 matches the finding, and a successful review matches the current
policy and tracked context. Changed or deleted source becomes a retained stale
finding requiring rescan/review, without a new task or devlog. A retry after an
already committed exact intake acknowledges its original task even if source
subsequently changed; that acknowledgement requires current triage before dispatch.

This sweep supports the mandatory style and independent frontier running-output
evaluator; it cannot replace either. Missing test/target/device/vision/audio
evidence remains inconclusive. Findings need reproducible source evidence and
meaningful acceptance criteria, never a numeric smell/coverage score alone.
See [Plan database](plan-database.md), [Coding style](coding-style.md), and
[ADR 0005](../ADR/0005-frontier-output-evaluation.md).
