# PIN-G07-SRD-001/a1 source acquisition evidence

Submitted source commit: `0df797256ae35350521fa9dc0d4ece8d5b85b4a0` (based on `f9343cb5ec17b33c76a03b20f6afec9168f9fb7a`); only the three listed worktree paths in that commit are changed. Acquired 2026-09-30 from the official English SRD v5.2.1 link shown on the publisher's [SRD page](https://www.dndbeyond.com/srd). The source PDF is retained unchanged at `development/rules-sources/srd-5.2.1-en.pdf` in the submitted worktree.

- Publisher: Wizards of the Coast LLC; title: *System Reference Document 5.2.1*; language: English; actual PDF pages: 364.
- Manifest SHA-256: `0defa673b21d19f98b10513b3c51c2a430d0aa3b3af339b57abfaa3d758fdeac`; verifier SHA-256: `3732f0f9be3853b2c7b7c16ace9c31f30ba4429f0f356973807d5790cf15ae29`.
- Bytes: 6,031,375; SHA-256: `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
- Publisher page currently calls this SRD v5.2.1 and links the English PDF; it says SRD 5.2/5.2.1 content is released under CC-BY-4.0 and identifies content excluded from the SRD. The [Creator FAQ](https://www.dndbeyond.com/creator-faq) is retained as context only; no additional book or rights are inferred from it.
- The PDF's actual page 1 contains its legal notice and exact required attribution. Printed/PDF pages 2–4 contain the contents and monster index. Contents entries verified from page 2 include Playing the Game 5, Character Creation 19, Classes 28, Character Origins 83, Feats 87, Equipment 89, Spells 104, Rules Glossary 176, Gameplay Toolbox 192, Magic Items 204, and Monsters 254.
- Visual evidence: `visual/page-001.png` through `visual/page-004.png`. SHA-256 values respectively: `049d52d4a7046145cc21a937e735900b3499b5ebe7972899396fd48f0e849b03`, `8cf9f4d842ac40444faa7a90868b3900464ca5462fcde817c6594349ca02374f`, `249c819674d18f6631467be61d5699eaf41a92320c857865300d6573dbf3dbcb`, `269d1c401f9d0830019f7d8d19370bbea7ad25b6a20cbb9a3176c84f850bf1f9`.

## Verification

- `pdfinfo development/rules-sources/srd-5.2.1-en.pdf`: PASS; page count 364; byte count 6,031,375.
- `python3 development/check-rule-source.py`: PASS; emitted `verified`, version 5.2.1, the SHA-256 above, 364 pages, partial SRD scope, and unacquired full-book sources.
- `python3` AST parse of `development/check-rule-source.py`: PASS.
- `git diff --check`: PASS.
- Bounded fixture outcomes: corrupted PDF bytes exited 1 (`source SHA-256 does not match manifest`); missing `language` exited 1 (`pinned metadata mismatch for language`); edition changed to 5.1 exited 1 (`pinned metadata mismatch for version`); omitted required full-book source exited 1 (`pinned metadata mismatch for book_titles`). Fixture data is temporary under `artifacts/tmp/PIN-G07-SRD-001-a1/fixtures/`.

## Explicit gaps

This submission is a partial SRD source pin. The 2024 *Player's Handbook*, 2024 *Dungeon Master's Guide*, and 2025 *Monster Manual* remain unacquired with no revision claimed. The full catalog denominator and non-SRD rights review remain unresolved. No full 2024 coverage, production `RulesetId`, or reviewed production `RightsGrant` is claimed. Required standard support remains open under the planning source and completion gates.

Scoped devlog entry: `pin-srd-g07-a1-source-pin-verified`.
