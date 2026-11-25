# UI-SHELL: worth a look later (not bugs)

- ui/src/keys/attach.ts:511-541 — Enter on any focused button in a dialog (for example 「キャンセル」 in a song dialog) presses the bright button instead, and Space plays / stops; this follows spec 3 ("Enter is the button at the right end") but leaves no key way to press the other buttons.
