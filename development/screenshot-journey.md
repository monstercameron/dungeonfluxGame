# Visual development journey

A working development tool captures the registered public DungeonFlux preview once
per active-development UTC minute. An owned bounded recorder process does the work
while agents code; the one-minute thread heartbeat checks/restarts that recorder
only during an already valid lease. Scheduled ticks never renew development activity.
The current target is the actual static project site in `docs/`, not a playable game.

## Coordinator work lifecycle

From the DungeonFlux directory, register a passive public target, start/renew the
lease when development work is actually active, and stop it when work ends:

```sh
python3 development/screenshot_journey.py register --kind static-site --label 'DungeonFlux public project-site preview (planned Rust game)' --public-safe
python3 development/screenshot_journey.py start --context /absolute/coordinator-issued-context.json --lease-seconds 1800
python3 development/screenshot_journey.py status
python3 development/screenshot_journey.py stop
```

Only real coordinator work renews the lease (60–1800 seconds). Long work renews it
before expiry; abandoned/idle work expires automatically. The recorder checks the
active lease each second between captures, coalesces duplicate requests in the same
UTC minute, and terminates on stop/expiry/disk cap. `recover` restarts an absent owned
recorder without extending the lease. `capture` can perform a bounded missed tick;
`index` rebuilds the timeline from retained records. Do not use OS cron or start a
second recorder. Subprocesses and temporary browser profiles are project-owned.

Capture cadence is best effort: process scheduling, load, browser launch or an app
that is closed/asleep can cause delay. Each record contains the actual observation
time and missed-minute count; no past screenshot is fabricated. Future recorded
minutes must be actual browser captures even if their content is unchanged.

## Preview scope and privacy

The static-site target serves only the project's `docs/` from a temporary owned
loopback server, closing it after capture. Screenshots use an isolated headless
Chrome profile, no desktop or user's browser, at a 1280×800 viewport. Only GET/HEAD
requests to the admitted local origin are allowed; external requests, service
workers and WebSockets are blocked. Nothing clicks links, submits forms, joins a
game, signs in, purchases or calls a provider. Fonts/media may use safe local
fallbacks when external fetches are denied.

A future `owned-preview` target requires an explicit coordinator-owned proof JSON:
`project_root`, exact `url`, live `owner_pid`, and true `public_safe`, `read_only`,
`synthetic_public`, `no_load_effects`. Register with `--url`, `--ownership-proof`
and one or more project-relative `--source-path`. Only credential-free loopback
HTTP URLs without query/fragment are admitted. Use a passive public/synthetic
presentation fixture; an ordinary game boot page that auto-joins or starts jobs
is unsuitable. Never register private phones, player secrets, sessions containing
personal data or a preview whose load causes gameplay/paid effects. The proof is a
trusted coordinator contract, not an automatic privacy classifier/security sandbox.

## Durable evidence and devlog

`development/evidence/journey/shots/` keeps actual immutable PNGs. `records/` keeps
UTC time, minute/session identity, preview/source before-and-after hashes, actual
browser/viewport/status, task/attempt, recorder identity, image hash/path and devlog
ingestion outcome. Changed sources during capture are disclosed rather than
claimed as a stable tested revision. `entries/` retains same-ID sanitized devlog
payloads; transient failures remain available for idempotent ingestion retry.
Devlog details stay below the append command's 16,000-character bound: they retain
the source identity digest and file count, before/after and changed-during-capture
facts, actual image hash, browser observation, task context and a reference to the
complete immutable record. The record retains every per-file source hash. When an
older pending entry exceeds the bound, preserve its original bytes and derive the
same logical JOURNEY ID into a deterministic `<record-id>.bounded.json` retry file;
accepted-size legacy payloads continue through their original path unchanged. An
existing devlog ID with different accepted content remains a conflict and is never
overwritten. Captured records are admitted only when both complete source maps have
valid per-file SHA-256 values, matching aggregate digests and a consistent
changed-during-capture flag. Malformed records or legacy entry JSON remain pending
without replacing their original bytes; unavailable and disk-cap outcomes may truthfully
omit capture-only source and image fields.

The recorder uses its own `screenshot-recorder:<session>` identity and the
coordinator-issued task context. Its own SQL attempt is null; the observed worker
agent/task/attempt remain explicit screenshot metadata, so the recorder cannot
impersonate an implementation attempt. Recorder contexts are immutable per session.
Older pending entries retry boundedly under their original context without recapture.
`development/devlog.py` performs the
parameterized append; screenshot observations never mutate queue status or approve
work. Failed/missing previews are recorded honestly without a fake image. Routine
photo entries are explicitly requested by the user, separate from problem analysis.

The retained journal has a 512 MiB cap and reserves up to 8 MiB per image. Reaching
the cap stops capture and records an actionable condition; it never deletes history
by age. Agents and cleanup must preserve this evidence, its active/config files,
entry payloads and referenced images. Temporary output lives in `artifacts/tmp/journey`.
A coordinator may archive evidence deliberately before resuming; automatic cleanup
cannot erase the journey.

The user-facing timeline is `outputs/dungeonflux-visual-journey.html`; the first
actual PNG is mirrored as `outputs/dungeonflux-journey-first.png`. Runtime logs,
workflow SQLite and the separate quality cache/heartbeat retain their distinct roles.

## Automation and runtime

Configured thread heartbeat: **DungeonFlux visual journey**
(`dungeonflux-visual-journey`), every minute in the existing development chat.
Keep the desktop app running and the machine awake for local scheduled work.
Timing is not a real-time guarantee. See the official
[scheduled-task documentation](https://learn.chatgpt.com/docs/automations?surface=app).

The tool uses the preinstalled bundled Node/Playwright and installed Chrome Beta;
no dependency was installed. Their exact local paths are saved in `target.json`.
A new host must verify its bundled paths/browser before capture; never install
or use a user profile implicitly. This is development tooling, not JavaScript game
application source or the future Rust workflow runner.
