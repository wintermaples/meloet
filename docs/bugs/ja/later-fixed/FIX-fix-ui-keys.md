# FIX-fix-ui-keys — 2 回目で直した点 (2026-11-25)

- ダイアログが開いている間も、メインのキーが背後で働きます。↑ ↓ ← → は選んだノートを動かし、Delete は選んだマーカー、点、クリップを消し、一文字のキーも働きます(`handleKeydown` in `ui/src/keys/attach.ts`)。今ダイアログを待つのは、出力の選択、棚、取り消しの帯だけです。
- `PresetMenu.svelte`: 開いたプリセットの一覧の中にフォーカスがあるとき、その `onkeydown` がすべてのキーを止めるため、Mod S、Mod Z、Space はそこでは働きません。
