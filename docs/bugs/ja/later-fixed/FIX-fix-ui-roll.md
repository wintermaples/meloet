# FIX-fix-ui-roll — 2 回目で直した点 (2026-11-25)

- ui/src/components/pianoroll/drag.ts:132-155 (`hover`) and ui/src/components/pianoroll/CcLanes.svelte:139-145 (`cursorAt`) — 録音中やカウントイン中は、押しても選ぶだけか、何もしない場所の上で、カーソルがまだ鉛筆、移動、サイズ変更の形を示します（描く道具も、和音の影を描き続けます）。
