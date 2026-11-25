---
id: MODEL-B-002
title: 鍵盤を音に変えても、鍵盤のトラックを手でミュートするだけで、動くミュートの欄があると、自分の音の上で鳴り続ける
area: MODEL-B
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# MODEL-B-002: 鍵盤を音に変えても、鍵盤のトラックを手でミュートするだけで、動くミュートの欄があると、自分の音の上で鳴り続ける

## Where

crates/meloet-model/src/audio/bounce.rs:137-141 (`bounce_track`: 鍵盤のトラックに `muted: true` を設定するだけ)
crates/meloet-core/src/play/mix.rs:50-56 (`silent_tracks`: 動いているミュートの欄が、手のミュートの代わりになる)
crates/meloet-core/src/play/bounce.rs:120-123 (`bounce_doc` は、書き出しでもミュートの欄を残す)

## What happens

鍵盤のトラックに、動いている (休んでおらず、点がある) ミュートの欄があると、「音のクリップにする」 / 「このトラックを音にする」 は、そのトラックの手のミュートを設定する。しかしエンジンは、ミュートの欄が動いているトラックでは、手のミュートを無視する。欄が「ミュートしない」と言っているところでは、鍵盤のトラックは鳴り続ける。書き出しでは同じミュートの欄を残したので、新しい歌のトラックは、まさにその場所で音が出る。そのため、同じ部分が 2 回鳴る (約 +6 dB、同じ音が重なる)。曲には、鳴っているトラックに対して `muted: true` が残る。

## What should happen

仕様書 3 章 (docs/spec/3-bands.md:455): 「鍵盤のトラックは (全部) ミュートされている」。ミュートの欄が動いているトラックにミュートを設定する兄弟の操作は、同じ手の中で欄を休ませるので、ミュートが聞こえる。`Play::toggle_mute` (crates/meloet-core/src/play/mix.rs:21-26, docs/architecture.md 21-11) と `recall_mute_set` (crates/meloet-model/src/mute_sets.rs:192-202、「その欄は同じ手の中で休む。そのトラックで M を押したときと同じく、そのセットが聞こえるように」) がそうである。

## How to trigger

コードを読んで分かったこと。実行はしていない。
1. 小節 1-8 にノートがある鍵盤のトラックと、点が 0.0 (小節 1) と 1.0 (小節 5) にあるミュートの欄を持つ (小節 1-4 が鳴り、小節 5 からミュートされる)。
2. トラックの見出しを右クリックし、「このトラックを音にする」。
3. 書き出し (`bounce_doc`) はミュートの欄を残すので、音には小節 1-4 があり、そのあとは無音になる。`bounce_track` は鍵盤のトラックに `muted: true` を設定するが、欄は動いたままにする。
4. 再生すると、小節 1-4 では鍵盤のトラック (その欄は「ミュートしない」と言っている。`laned` が真なので `silent_tracks` は手のミュートを飛ばす) と新しい歌のトラックの両方が鳴る。

## Cause

`bounce_track` は鍵盤のトラックを `Track { muted: true, ..source }` で作り、その `AutoTarget::Mute` の欄には触れない。`silent_tracks` は、トラックを手のミュートで無音と扱うのは `!laned` (動いているミュートの欄がない) のときだけなので、この手順が設定した手のミュートは、欄が動いている間は効かない。

## Suggested fix

`bounce_track` で、鍵盤のトラックのミュートの欄が動いているとき、同じ手の中でその欄も休ませる (トラックを置き換えたあとに `automation::set_resting(&mut new, from, AutoTarget::Mute, true)`)。`recall_mute_set` と同じ。取り消すと両方が戻る。プロトコルや曲ファイルの形は変えない。

## Verification

コードを読んで確認した (テストは実行していない)。`bounce_track` (`crates/meloet-model/src/audio/bounce.rs:137-141`) は `muted: true` だけを設定する。`Play::silent_tracks` (`play/mix.rs:50-56`) は、動いているミュートの欄がないトラックでだけ手のミュートを数える。`bounce_doc` (`play/bounce.rs:104-123`) は、フェーダー、パン、センドの欄だけを外すので、書き出しもミュートの欄に従う。書いたとおりの結果になる。欄が「ミュートしない」と言っているところでは、鍵盤のトラックもその音も鳴る。仕様書 3 の A7: 「鍵盤のトラックは (全部) ミュートされている」。重大度 `medium` は変えない。

訂正: なし。

修正案: `crates/meloet-model/src/audio/bounce.rs` の `bounce_track`: 鍵盤のトラックに `moves()` の `AutoTarget::Mute` の欄があるとき、同じ手の中で休ませる (`mute_sets.rs:192-202` と同じやり方)。テスト: モデルの bounce テストで、手順のあと欄が休むこと、取り消すと動く欄に戻ること。`crates/meloet-core/src/play/bounce/tests.rs` で、手順のあと `silent_tracks` が鍵盤のトラックの席を無音として保つこと。

## Fix

`crates/meloet-model/src/audio/bounce.rs` の `bounce_track`: 鍵盤のトラックのミュートの欄が動いているとき、同じ手の中で休ませる (`automation::set_resting`。`recall_mute_set` と同じ)。その doc コメントにも書く。`docs/architecture.md` 25-31 にも同じことを書く。テスト: `audio::bounce::tests::a_mute_lane_that_moves_rests_in_the_same_step_so_the_mute_is_heard` (モデル: 手順のあと欄が休み、取り消すと動く欄に戻る) と `play::bounce::tests::the_keys_track_is_silent_after_the_step_under_its_mute_lane` (コア: `bounce_made` のあと、鍵盤のトラックの席が無音だとエンジンに伝えられる)。
