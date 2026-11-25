# fix-ui-lanes — seen while fixing, not fixed

- ui/src/components/NumberField.svelte — if a box is still open when the count-in or the recording starts, it stays open and still takes Enter (core refuses the value, and an out-of-range one still sends `clampValue`); the new `disabled` only stops opening it. Not tried whether anything can start the count-in while a box has the focus.
