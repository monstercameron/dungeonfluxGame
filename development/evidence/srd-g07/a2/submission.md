# PIN-G07-SRD-001/a2 repair handoff

Submitted worktree commit: `6856a7457ad335028ccbe787ef01867f6e07c156`, parent `0df797256ae35350521fa9dc0d4ece8d5b85b4a0`. Relative to the parent, only `development/check-rule-source.py` changed. The worktree is clean. The acquired PDF and manifest are byte-identical to a1:

- `development/check-rule-source.py`: SHA-256 `9e72681ee249e7855de17dbbf945a247ac3052404ead78074acb78c3a4aab1f3`
- `development/rules-sources/manifest.json`: SHA-256 `0defa673b21d19f98b10513b3c51c2a430d0aa3b3af339b57abfaa3d758fdeac`
- `development/rules-sources/srd-5.2.1-en.pdf`: 6,031,375 bytes; SHA-256 `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`

The verifier now requires the exact complete pinned attribution statement. Partial SRD integrity remains a successful and explicitly partial result. `--require-full-book-sources` separately refuses qualification if any required PHB/DMG/MM source status, revision, local file pin, byte size, or digest is unavailable or mismatched; it does not claim full rules coverage or rights clearance. Manifest input is opened nonblocking, must be a regular file, and is capped at 1 MiB before JSON parsing. Source PDFs retain the 64 MiB bound. Read failures after a size check are converted into bounded verifier refusals.

## Verification

Results are retained in `artifacts/tmp/PIN-G07-SRD-001-a2/verification/verification-results.json` and `artifacts/tmp/PIN-G07-SRD-001-a2/final-checks/verification-results.json`.

- `python3 development/check-rule-source.py`: exit 0, verified SRD v5.2.1 SHA/size/364 pages, explicitly partial; full book sources reported unacquired.
- `python3 development/check-rule-source.py --require-full-book-sources`: exit 1, safe refusal: `2024 Player's Handbook bytes are unavailable`.
- Corrupted PDF bytes: exit 1, SHA-256 mismatch. Missing language metadata: exit 1. SRD 5.1 label: exit 1. Omitted full-book record: exit 1.
- Empty attribution and `This work`: both exit 1, exact pinned attribution mismatch.
- Records marked acquired without source files: `--require-full-book-sources` exits 1, file pin unavailable.
- Malformed JSON, manifest larger than 1 MiB, and FIFO manifest: each exits 1. FIFO response was immediate with `manifest must be a regular file`; no external timeout was needed.
- Python AST syntax check: PASS. `git diff 0df797256ae35350521fa9dc0d4ece8d5b85b4a0..HEAD --check`: PASS. Changed path list contains only `development/check-rule-source.py`; worktree status is clean.
- Existing a1 visual inspection remains applicable because the PDF bytes are unchanged: `../visual/page-001.png` through `../visual/page-004.png`. Original publisher and PDF provenance is retained in `../submission.md` and the original source manifest.

No full-book bytes or revisions were acquired or fabricated. Full 2024 coverage, catalog denominator, non-SRD RightsGrant review, and production RulesetId remain unresolved. Scoped a2 devlog entry: `pin-srd-g07-a2-verifier-bounds-repaired`.
