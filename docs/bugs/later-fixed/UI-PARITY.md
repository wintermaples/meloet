# UI-PARITY — fixed in the second pass (2026-11-25)

- ui/src/components/mixer/MuteSetBox.svelte:79-96 — the name box is moved to body and stops its keys with stopPropagation only, so keys/attach.ts still sees Enter (e.g. chooserConfirm while the output chooser is shown); MetronomeWindow.svelte:130-134 uses stopImmediatePropagation for the same case (same root as UI-SHELL-004).
- ui/src/components/LatencyDialog.svelte:182-185 — the dialog body scrolls with `overflow: auto` (the browser's bar) while the other dialogs use `ScrollArea` (spec 2 "Scroll bar"); its content is short, so it may never overflow at 1024×640.
- ui/src/components/SongDialog.svelte:17, LatencyDialog.svelte:56, export/ExportDialog.svelte:146, plugins/ScanStoppedDialog.svelte:29 — no `style:color-scheme={metrics.colorScheme}`, which SettingsDialog, AboutDialog, KeyListDialog and ExportedDialog set (native parts of text boxes follow the theme only in those); no visible effect confirmed.
- ui/src/components/params/StretchNumber.svelte:117-120 — a disabled pitch / 「音程を寄せる」 number keeps the `ns-resize` cursor, while a disabled parameter bar gets `cursor: default` (ParamRow.svelte:297-299).
- ui/src/components/params/knobRows.ts:42 — `replace("−", "-")` maps only the first U+2212 (harmless for one minus sign).
- ui/src/components/ChoiceMenu.svelte:69-71, ZoomMenu.svelte:37-39, WindowsMenu.svelte:27-28 — with the focus on the list box itself (no row), ↑ goes to the first row; MenuList.svelte:101-102 goes to the last row. Reached only when the first focus did not land on a row.
