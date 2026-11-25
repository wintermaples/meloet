# VERIFY-RUST — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/export/job.rs:1083 — `move_to` renames the finished export over whatever file has that name. When the person never pressed 「えらぶ…」 (the usual place and the song's name), an earlier export of the same name is replaced with no question; GTK's question is asked only inside the save window. Not checked against spec 3 (the export dialog); met while checking CORE-EXPORT-002.

## not true when looked at again (2026-11-25)

- crates/meloet-core/src/songfile.rs:1896 — a song file already written by the fault of MODEL-A-001 (a note with an origin index in a take with no origin) stays unreadable after that fix. The reader could take such a note as a plain note instead of refusing the song; a choice for the lead.
