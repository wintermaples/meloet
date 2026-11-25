# CORE-PLAY-B — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/play/automation.rs:753 と play/mixer.rs:256-266 — 離すとき、レーンの `HoldLane` の off が、残りの手(rest または write step)の `SetAutomation` より先に送られます。二つの間にオーディオスレッドがコマンドのリングを空にすると、古い線が一ブロックの間鳴ります(線へ向かう短いランプと、戻るランプ)。
- crates/meloet-core/src/play/vocal.rs:291-322 — `voices_sent` は席ごとに保持され、`clear_seat` はそれを忘れません。一回の変更でボーカルトラックの席が空になり、同じ入力と `on` を持つ別のボーカルトラックに渡されると、`ClearTrack` がエンジンの voice を捨てたのに、`Voice` は送られません(`on` が false の間は音への影響は見つかっていません。履歴のジャンプでは起こり得ます)。
- ui/src/components/TrackHeads.svelte:270-271 — トラックの見出しの M は、手で設定した `track.muted` を表示します。一方、ミキサーの M(`MixerStrips.svelte:99-103`)は、動いているミュートのレーンの値を表示します。同じトラックの二つの M が食い違うことがあります(仕様は、レーンの下にある見出しの M については何も書いていません)。
