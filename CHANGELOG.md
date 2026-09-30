# Changelog

Record every commit integrated into this project in six-hour work blocks.
Start each block when development work begins, close it after six hours, and
close a shorter final block when the work session ends. Put the newest block
first and list its commits in integration order.

Each block records its start and end time with timezone offsets, starting and
ending Git revisions, and one entry per commit: abbreviated hash and commit
subject. Include the task ID when available. Use the revision range to capture
all integrated commits, including commits originally authored outside the block.
Do not filter by author timestamp or include the archived `dungeonflux.old` repo.

For the first block there is no prior revision: mark the baseline as empty and
include every commit reachable at its ending revision, including the root commit.
Subsequent blocks use the previous ending revision as their starting revision.

The coordinator is the single writer. Record commits once; leave completed
blocks unchanged unless correcting an error. A block with no commits says so.

<!-- Block format:
## YYYY-MM-DD HH:MM +/-HH:MM to YYYY-MM-DD HH:MM +/-HH:MM

Range: <starting revision>..<ending revision>

- <commit hash> — <commit subject> (task <ID>, if available)
-->

## Pending first work block

No commits yet. This new project has not been initialized as a Git repository.
