---
id: PLUG-002
title: Scan stamp of a plugin bundle that is a symbolic link never changes, so a changed or removed linked plugin is never scanned again
area: PLUG
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# PLUG-002: Scan stamp of a plugin bundle that is a symbolic link never changes, so a changed or removed linked plugin is never scanned again

## Where

crates/meloet-plugin/src/scan.rs:127-155 (`BundleStamp::read` / `add`: `std::fs::symlink_metadata` on the bundle itself)
crates/meloet-core/src/plugins/scanner.rs:252-253 (`bundles_in`: a bundle is found by the ending of its name, a link included)
crates/meloet-core/src/plugins/scanner.rs:269-276 (`places_todo`: an unchanged stamp leaves the record as it is)

## What happens

A bundle in a plugin place that is itself a symbolic link (for example `~/.vst3/Foo.vst3 -> ~/Downloads/Foo/Foo.vst3`, or `~/.lv2/foo.lv2 -> /opt/foo/foo.lv2`) is found and scanned once, but its stamp is only the link's own modified time and the length of the link text. Whatever happens to the folder the link points to, the stamp stays the same, so:

- after the person updates the plugin in place (new plugins in the bundle, a plugin removed, a new version with other parameters or another kind), meloet keeps the old record: new plugins never reach the shelf, removed ones stay on it;
- after the target is deleted (the link now points nowhere), `symlink_metadata` still succeeds on the link, `bundles_in` still finds the name, and the plugin stays on the shelf; choosing it fails to load.

Only 「もう一度スキャン」 of a stopped plugin or deleting the scan cache would read it again.

## What should happen

docs/architecture.md 20-4: "Results are remembered by path, modified time and size; later scans look only at changed bundles." A bundle whose contents changed must count as changed. The note "Links are not followed" (scan.rs:126) is about links inside the bundle while walking it; the bundle the scan actually reads through the link is the target folder.

## How to trigger

Read from the code, not run.
1. Put a plugin folder somewhere and link it into a place: `ln -s /opt/x/Foo.vst3 ~/.vst3/Foo.vst3`. Start meloet; it is scanned.
2. Replace the contents of `/opt/x/Foo.vst3` with another version that adds a second plugin class (or delete `/opt/x/Foo.vst3`).
3. Start meloet again: `BundleStamp::read(~/.vst3/Foo.vst3)` gives the same stamp (`symlink_metadata` of the link: not a dir, size = link length, link mtime), `places_todo` returns `None`, the old record stays.

## Cause

`BundleStamp::add` calls `std::fs::symlink_metadata(path)` also for the first path, the bundle. For a link that is not a directory, so the walk stops there and the stamp is the link's own. `bundles_in` accepts a link by its name ending without looking at what it is.

## Suggested fix

Read the bundle itself with `std::fs::metadata` (following a link at the top only) and keep `symlink_metadata` for what is inside, for example by passing a flag for the first level in `BundleStamp::read`. A dangling link then gives an error, and `places_todo` would leave its old record in place, so it must also count as gone: in `bundles_in` (scanner.rs, another area) skip an entry whose `std::fs::metadata` fails, which puts it in `gone`. No protocol or file-shape change; `SCAN_CACHE_VERSION` need not change (only links' stamps differ, and they then read as changed once).

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
