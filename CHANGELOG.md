# Changelog

Record every integrated commit once, in six-hour work blocks. Close a shorter
final block when the work session ends; put the newest block first and retain
completed blocks. Record timezone-aware boundaries, starting/ending revisions,
and commit hash/subject in integration order, including merges and root commits.
Use revision ranges, not author-time filtering. The first baseline is empty;
subsequent blocks begin at the prior ending revision. Exclude dungeonflux.old.
The coordinator is the single writer.

A changelog commit cannot record its own hash. Close a block at the reviewed
integration commit, then commit this record separately. Carry that bookkeeping
commit into the next block: keep the recorded ending revision as the next
baseline and capture baseline..HEAD, which includes the bookkeeping commit.
Do not invent a self-hash or silently discard metadata commits.

## 2026-09-30T01:13:16-04:00 to 2026-09-30T02:58:24-04:00

Range: empty baseline..12dba460ba1a3d3a5566e1074938520041da2b34

- cd0c8af — Init commit
- 12dba46 — Refine reviewed service contracts and preserve development evidence

Next block baseline: `12dba460ba1a3d3a5566e1074938520041da2b34`. The commit writing this block remains pending
for capture in that next block. This final work-session block is shorter than six hours.
