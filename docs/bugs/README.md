# Bug reports

Findings of a sweep over the code (`crates/` and `ui/`): bugs, UI/UX problems, and things one side has and its sibling does not.

- One file per finding in [`reports/`](reports/), all in the form of [`TEMPLATE.md`](TEMPLATE.md).
- Make a report with `python3 -I tools/bugs/report.py new --area <AREA> --kind bug|ui-ux|parity --severity high|medium|low --title "<one line>"`, then fill the sections. Do not copy the template by hand.
- `python3 -I tools/bugs/report.py index` checks every report and writes [`INDEX.md`](INDEX.md).
- `python3 -I tools/bugs/html.py` writes [`report.html`](report.html): every report and every "later" line on one page for reading in a browser (search, filters by severity, kind and area; `report.html#<ID>` opens one report). Run it again after reports change.
- Japanese translations of the reports, the "later" lines and this file are in [`ja/`](ja/) (same file names; made by helpers on 2026-11-25, checked by machine only for shape and code spans). `python3 -I tools/bugs/html.py --lang ja` writes [`report.ja.html`](report.ja.html) from them. The English files are the source: when a report changes, its translation must be changed too.
- Things that are not bugs but are worth a look later (slowness, spec and mock disagreeing, design worries) go in [`later/`](later/), one file per area, one line each. They are not fixed in the sweep.

- Lines of [`later/`](later/) that were fixed after all are moved to [`later-fixed/`](later-fixed/) (one file per area; lines that turned out not to be true are under their own heading there).

## Fields

- `kind`: `bug` (wrong behavior), `ui-ux` (works, but a person is confused, blocked or misled), `parity` (one side has it and its sibling does not: similar screen parts, Rust and the screen, Japanese and English wording).
- `severity`: `high` (data lost or damaged, sound stops or is wrong, a crash, a hang, a part that cannot be used), `medium` (wrong behavior a person meets in normal use, with a way around), `low` (rare corner, small visual or wording fault).
- `status`: `reported` (found, not checked) → `confirmed` (the verifier checked it is real) → `fixed`. `needs-lead`: real, but the fix needs a decision by the lead. Reports found to be false are deleted.

## Sweeps

- 2026-11-25: the first sweep (branch `fix/bug-sweep`): 114 found, 110 true, 110 fixed. What is left open is in `docs/progress.md`, section 6, "Bug sweep".
- 2026-11-25, second pass (the same branch): the owner asked for the later list to be gone through. Of its 193 lines, 112 were fixed and 4 were not true; 77 stay, and 41 new lines seen on the way are in `later/PASS2.md`. The owner's decisions and the lead's are in `docs/spec/7-readings.md`.
