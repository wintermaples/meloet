---
id: ENGINE-002
title: リターンとマスターのプラグインには、ピースの最初のフレームではなく、最後の区切りでの再生位置とテンポが伝わる
area: ENGINE
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# ENGINE-002: リターンとマスターのプラグインには、ピースの最初のフレームではなく、最後の区切りでの再生位置とテンポが伝わる

## Where

crates/meloet-engine/src/engine.rs:2964-2969 (`set_transport` は区切りごとに書かれ、組み込みエフェクトの `ctx` だけが最初のもの(`piece_ctx`)を保つ)
crates/meloet-engine/src/engine.rs:3025-3039 (リターンのインサートは、区切りのループのあとで動く)
crates/meloet-engine/src/engine.rs:3064-3065 (マスターのインサートは、区切りのループのあとで動く)
crates/meloet-engine/src/plugin.rs:211-216 (`submit` は、その時点の `self.block.transport()` をそのままコピーする)

## What happens

`Engine::render` は、ピース(一回に処理する音のかたまり)を区切り(ノート、クリック、テンポの点、ループの終わり、アルペジオの段、ミュートの段などで切る)に分け、区切りごとに「区切りの最初のフレームで曲がどこにあるか」を共有の `PluginBlock` に書く(2965 行目)。トラックの音源とインサートは区切りのループの中で動き、正しい値を読む。リターンとマスターは、ピース全体に対して一度だけ、ループのあとで動く(2964-3065 行目)。そのため、そこにあるプラグインのエフェクトは、最後の区切りが `PluginBlock` に残した値を提出する。その `position_quarters`、`position_frames`、`bar`、`bar_start_quarters`、テンポ、フラグは、ピースの中のどこかのフレームを指している。一方、プラグインはピースを最初のフレームから処理する。

- 曲の再生中は、伝わる再生位置が、ブロックごとに 0 から 1 ブロック(最大 1024 フレーム)ぶん前後に飛ぶ。それは最後の切れ目がどこに落ちたかによる。リターンかマスターにあって、曲の再生位置に合わせるプラグイン(曲の位置に同期した LFO、ゲート、トレモロ、ディレイ)は、その位置と同じ周期で揺れる。(LV2 のプラグインは、この値から、ブロックごとにフレーム 0 で `time:Position` を受け取る。docs/architecture.md 21-3)
- ループの終わりを含むピースでは、ブロックの前半がまだループの終わりより前にあっても、プラグインはループの始まりを伝えられる。
- カウントインが終わるピースでは、最初のフレームから「録音中」と伝えられる。

同じストリップの組み込みエフェクトは最初の区切りのテンポ(`piece_ctx`)を受け取るので、二つの種類のエフェクトが食い違う。

## What should happen

docs/architecture.md 21-3 では次のとおり。「組み込みのディレイ: `EffectContext::tempo_bpm` は、区切りの先頭の `tempo_at`。リターンとマスターは区切りで分けないので、レンダリングの先頭(最大 1024 フレーム)の値を受け取る」「プラグインへ(`PluginTransport`): 区切りの先頭の値」。区切りで切られないストリップは、区切りが一つ(ピース全体)で、その先頭はピースの最初のフレームになる。組み込みエフェクトは、これをすでに `piece_ctx` で受け取っている。

## How to trigger

コードを読んで確かめたもので、実行はしていない。

1. 曲の位置に合わせるプラグインのエフェクト(例: テンポに同期したトレモロ、同期モードの LFO で動くフィルター)を、リターン A かマスターに置く。
2. ノートのあるトラックを持つ曲を再生する。ほとんどのブロックが、複数の区切りに分かれるようにする。
3. プラグインは、ブロックごとに違う量だけ遅れた位置を伝えられ、その同期したモジュレーションがカクつく。同じプラグインをトラックのインサートに置くと、滑らかに動く。

## Cause

`PluginBlock` は一つのトランスポートだけを持ち、区切りの先頭ごとに上書きされる。`PluginSlot::submit` はそれを、プラグインのブロックを渡すときに読む。リターンとマスターでは、それは最後の区切りのあとでだけ起きる。

## Suggested fix

最初の区切りの `told`(最初の繰り返し)を `piece_ctx` の隣に保ち、リターンのインサートが動く直前(3025 行目)に `self.plugin_block.set_transport(&first)` で書き戻す。ほかは変えない。プロトコルもファイルの形も変えない。`tests/lookahead.rs` の `plugins_are_told_where_the_song_is` に、マスターの偽のプラグインのエフェクトを足して、伝えられた値を記録させられる。

## Verification

読んで確かめた(実行はしていない)。`Engine::render` では、区切りのループが区切りの先頭ごとに `self.plugin_block.set_transport(&told)` を書く(`engine.rs:2964-2965`)。そして、組み込みエフェクトの `ctx` だけが最初のもの(`piece_ctx`)を保つ。リターンとマスターのインサートは、ループのあとで動く(`engine.rs:3025-3065`)。プラグインのエフェクトの `submit`(`plugin.rs:211`)は、そのときの `self.block.transport()`、つまり最後の区切りの値をコピーする。アーキテクチャの 21-3 は、リターンとマスターが「レンダリングの先頭の値を受け取る」と書いている。深刻度 `low` はそのまま。伝わる位置のずれは 1 ブロック未満で、リターンかマスターにあり、曲の位置に合わせるプラグインだけにその影響が出るため。

訂正: なし。

修正計画: `crates/meloet-engine/src/engine.rs` の `render`: 最初の区切りの `told` を `piece_ctx` の隣に保ち(`Copy` の値で、確保はしない)、リターンのループの前に一度 `self.plugin_block.set_transport(&first)` を呼ぶ。テスト: `tests/lookahead.rs` の `plugins_are_told_where_the_song_is` に、複数の区切りに分かれたブロックの、マスターの偽のプラグインのエフェクトを足す。

## Fix

- `crates/meloet-engine/src/engine.rs` の `render`: 最初の区切りのトランスポートを `piece_ctx` と一緒に保ち、リターンとマスターのインサートが動く前に `PluginBlock` へ書き戻す。
- テスト: `crates/meloet-engine/tests/lookahead.rs` の `plugins_on_the_returns_and_the_master_are_told_the_head_of_the_piece`(リターン A とマスターの偽のプラグインのエフェクトが、プラグインのスロットの `submit` と同じように `PluginBlock` を読む。100 フレームごとにキーで切られたブロックで)。
