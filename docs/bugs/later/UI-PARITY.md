# Later: UI-PARITY

- ui/src/components/pianoroll/paint.ts:583-597 — a moved CC point keeps a gray.7 border at its original place, while automation and tempo points keep an accent one (spec 2 "Automation point", 7-readings A-20); spec 2 says nothing of CC points, so the lead may want one rule for all small points.
- ui/src/components/** — no drag part listens for lostpointercapture or window blur; a drag whose pointerup never comes (window loses focus mid-drag) stays open until the next press. Consistent across parts, so not a parity gap, but worth one shared rule.
- ui/src/components/params/PresetMenu.svelte:57-69 — the preset list stops every key but has no ↑ ↓ between its rows, while the ▾ lists (ChoiceMenu, ZoomMenu, WindowsMenu, MenuList) move with ↑ ↓; 7-readings M1 says it is drawn in the panel, so it may be meant to differ.
- ui/src/components/params/ClipPanel.svelte:124-129 — unreadable text in the clip 「ゲイン」 box sends nothing and says nothing, while the position, length and fade boxes of the same group send `rejectValue` (the code comment of ClipsPanel says it is meant for gain).
