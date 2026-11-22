# LV2 へオートメーションとテンポをわたす作法

調べた日 2026-11-20（meloet-research）。確かさは A・B・C。[S番号] は最後の表。LV2 の仕様の版は、この PC に入っている物（手もとの実物）の版。

## 結論（先に）

1. **control ポートは、1 回の `run()` のあいだ 1 つの値で動かない**（A [S1]）。ブロックの中で値を変えたければ、ホストが `run()` を何回かに分けて呼び、1 回ごとに値を入れなおすしかない。仕様は分けることを禁じていない。ブロックの長さは「ホストが決める、いつも同じとは限らない」と書いてある（A [S1]）。Ardour は実際に、オートメーションの点でブロックを分ける。分ける最小の長さの決まりは持っていない（A [S11]）。
2. **プラグインが「分けないで」と頼む印がある**: `bufsz:coarseBlockLength`（`lv2:optionalFeature` で言う）。仕様は「コントロールが変わったからといって run を分けないでほしい」という意味で、**ただの目印（強制ではない）**と書く（A [S3]）。Ardour は、この印のあるプラグインは分けない（A [S12]）。
3. **`patch:Set` の時刻（`frames`）は、仕様に特別な決まりがない**。atom:Sequence の時刻はブロックの頭からのこま数（A [S4]）。しかし **Surge XT の LV2（JUCE の包み）は、`patch:Set` の時刻を見ない**。ブロックの頭で、列の順に全部当ててから、ブロック全体を 1 回で処理する（A [S9]）。**時刻が効くのは MIDI だけ**。Surge の中のつまみを、ブロックの途中で変える道は、ブロックを分けて呼ぶ以外にない。
4. **`time:Position` は、いつ送るかを仕様が決めていない**（A [S6]）。jalv と Ardour は「変わったときだけ」、時刻 0 で送る（A [S16] [S12]）。**JUCE の包みは、毎ブロックの頭で位置を空にして、そのブロックの中に `time:Position` が来たときだけ作る**（A [S9]）。だから JUCE のプラグインには**毎ブロック**送らないと、テンポも拍の位置もわからなくなる。JUCE は `time:beat` を読み、`time:barBeat` は読まない（A [S9]）。
5. **`ui:touch`（`LV2UI_Touch`）は、control ポート用**（番号で指す）。patch のパラメータの「さわっている」を知らせる仕組みは、仕様にない（A [S5] [S7]）。**Surge XT の窓は、`ui:touch` を使わない**（ui.ttl に宣言がなく、JUCE の窓は一度も呼ばない。A [S8] [S9]）。
6. **ホストが値を変えたとき窓に知らせる道**は、control ポートなら `port_event`（形式 0、float 1 つ）。atom は `atom:eventTransfer` の形式で `port_event`（A [S7]）。**Surge XT の窓は、`port_event` を何もしない**。窓がプラグインの中の物（instance-access）を直接見ているので、知らせなくても窓の値は変わる（A [S9] [S8]）。

## 1. control ポートの値をブロックの途中で変える

### 仕様（A）
- 「control ポートは 1 つの `float` で、**1 回の `run()` のあいだ固定で有効**。だから control のレートはブロックの大きさで決まり、それはホストが決める（いつも同じとは限らない）」（core 18.6 の説明 [S1]）。
- `run(sample_count)`: 「プラグインが走る 1 ブロックの大きさ」。`sample_count` が 0 のときは、出力の control（遅れなど）を更新する特別な場合（lv2.h [S2]）。分けてよい、とも、いけない、とも書いていない。
- `buf-size`（1.4 [S3]）: `minBlockLength` は 0 でもよい。`fixedBlockLength`（いつも同じ長さ）を必須にするプラグインは、分けられない。**`coarseBlockLength`**: 「粗くて規則正しいブロックを好む。例えば、サンプル単位のコントロールを持たないプラグインが、コントロールが変わったからといってホストが run を分けないでほしいと言うために使う。ただの目印で、固定の長さの保証ではない。run は他の理由でも分けられ、ブロックの長さはいつでも変わりうる」。
- 音のレートでなめらかに変えたい値のために、`lv2:CVPort`（音と同じ形の配列）がある（A [S1]）。プラグインがその口を持つときだけ使える。

### ホストのやりかた
| ホスト | 作法 | 確かさ |
|---|---|---|
| **Ardour** | `PluginInsert::automate_and_run`: 自動の「次の点」を探し、点までを 1 回の `connect_and_run` にする、を残りがなくなるまでくり返す（`cnt = min(次の点までの距離, nframes)`、`nframes -= cnt; offset += cnt`）。**分けないとき**: プラグインが `requires_fixed_sized_buffers()`（LV2 では `coarseBlockLength`、古い `noSampleAccurateCtrl`）か VST3 のとき、コントロールのロックが取れないとき。分けない場合、点の値は「サイクルの頭の値」にそろえる。1 回分けた中で、control の値は、その区間の頭の時刻で自動の線を読んで入れる（`rt_safe_eval(start)`）。atom の列は、分けなくても時刻のまま | A [S11] [S12] |
| **最小の長さ** | Ardour に**最小の長さの決まりは見つからない**。歯止めは、割りこみのくり返し回数（`timeout = 8`）と、`cnt <= 0` で抜ける所だけ | A [S11]（要約を通して読んだ） |
| **jalv** | 自動の機能がない。UI から来た control の値をサイクルの頭で入れるだけ | B [S16] |
| **Carla** | `processSingle(…, frames - timeOffset, timeOffset)` の形で、イベントの時刻で区切るらしい。分ける条件の所は読めなかった | B [S17] |

### この PC の実物（A [S20]）
`nodelay.lv2`（Ardour の作者の物）は `lv2:optionalFeature … bufsz:coarseBlockLength` を宣言している。Surge XT は control ポートのつまみを 1 つも持たない（0 個）ので、ここは当たらない。

### livi 0.9.0 と meloet（A [S18] [S19]）
meloet は `min_block_length: 1` で渡している。1 こままで分けても、livi は断らない（livi は `sample_count` が最小より小さいと誤りを返す）。

## 2. `patch:Set` を atom の列で送る

- `atom:Sequence` の時刻: 本体の `unit` が時刻の単位。0 なら「文脈で決まる」、`run()` の列ならこま（frames）（A [S4]）。「atom:Sequence を持つ AtomPort は、MIDI のようなイベントを**サンプル単位で**やりとりするのに使える」（A [S4]）。**並びの順（時刻順）を決める文は、読んだ範囲では見つからなかった**（A、見つからない事の確認）。
- `patch:Set`（patch 2.10 [S5]）: 「その property の古い値を全部消して、新しい値を入れる `patch:Patch` と同じ」。**時刻や順序の決まりは書いていない**。`patch:property` と `patch:value` は 1 つずつ。
- 同じ property の `Set` を 1 ブロックに何個も入れてよいか: 禁止の文はない。受け取る側が列の順に読めば、最後の値が残る（C、推論。JUCE の包みはその通りに動く [S9]）。
- ホストの実際: JUCE の包みは時刻を見ない（3 章）。Ardour は、ホストからの `patch:Set`（`set_property`）を、ブロックの**最後のこま `nframes - 1`** に書く（B [S12]。要約を通して読んだ）。

## 3. Surge XT の LV2（JUCE の包み）は `patch:Set` の時刻を守るか

**守らない（A）**。読んだ物: Surge の `release_xt_1.3.4` の `libs/JUCE`（submodule、`surge-synthesizer/JUCE` の `cf5754b19c87…`）の `juce_audio_plugin_client_LV2.cpp` [S9] [S10]。手もとの Surge XT 1.3.4 が、この版でビルドされたことは、バイナリでは確かめていない（C）。

- `run()` は、まず `midi.clear()`、`playHead.invalidate()`、音のバッファの大きさを `numSteps` に直し、そのあと**入力の列を最後まで 1 周**して、各イベントに `patchSetHelper.processPatchSet (event, Callback {*this})`、`playHead.readNewInfo (event)`、MIDI なら `midi.addEvent (…, event->time.frames)` をする。**そのあと `processBlock` を 1 回だけ**呼ぶ。
- `patch:Set` の `setParameter` は `parameters.setValueFromHost (property, value)` に行くだけで、`event->time.frames` を見ない。ブロックの中で区切って処理する所もない。
- `setValueFromHost` は、`ignoreCallbacks` を立てて `setValueNotifyingHost` を呼ぶ。そのため、ホストが入れた値は、プラグイン側の出力の `patch:Set` として**はね返らない**（A [S9]）。
- つまみは、全部 `patch:Set` で来る（`rdfs:range atom:Float`、`patch:writable`）。control ポートで読むのは、`freeWheeling`（入力）、`enabled`（入力）、`latency`（出力）だけ [S9] [S8]。
- 入力の atom ポート `in`: `atom:supports midi:MidiEvent, patch:Message, time:Position`、`rsz:minimumSize 63992`、`lv2:designation lv2:control`（A [S8]）。
- Surge の中身が、JUCE から受けたつまみを、さらにサンプル単位で扱うかは調べていない（わからない）。ただし包みの段階で時刻が捨てられるので、LV2 では時刻は届かない。

## 4. `time:Position` の送りかた

### 4-1. 欄の意味と型（A [S6]。time 1.6）
| 欄 | 型 | 意味 |
|---|---|---|
| `time:frame` | long | 音のこま数での時刻（曲の頭からの位置と読むのが自然。C） |
| `time:speed` | float | 進む速さ。0.0 は止まっている、1.0 は普通、0.5 は半分、−1.0 は逆 |
| `time:bar` | long | 小節の番号 |
| `time:barBeat` | float | 小節の中の拍。「0 から `beatsPerBar` まで」。0 始まり |
| `time:beat` | double | 「通しの拍の数」。`time:bar` と同じ原点から数え、**止めて動かさない限り、増えつづける**（繰り返しや頭出しがなければ） |
| `time:beatUnit` | nonNegativeInteger | 1 拍にあたる音符。拍子の下の数（2 なら 2 分、4 なら 4 分） |
| `time:beatsPerBar` | float | 1 小節の拍の数（拍子の上の数） |
| `time:beatsPerMinute` | float | テンポ（1 分の拍の数） |

- 0 始まりの扱い: `barBeat` は仕様が 0 始まり。`bar` は、Ardour が「小節番号 − 1」で書く（B [S12]）。仕様に `bar` の始まりの文はない。
- `time:Position` の中の「拍」は、`beatUnit` の音符を 1 と数える（A、`beatUnit` の説明から）。8 分の 6 拍子なら、`beat` は 8 分音符の数。JUCE の包みは `time:beat` を、そのまま ppq（4 分音符の数）として渡す（A [S9]）ので、`beatUnit` が 4 でないとき、4 分音符の数とずれる（C、推論）。`time:scale` は Ardour が足す欄で、仕様 1.6 にはない（A [S6] [S12]）。

### 4-2. いつ送るか（仕様は決めていない。A、見つからない事の確認）
| 実装 | 送る時 | 時刻 | 確かさ |
|---|---|---|---|
| **jalv**（JACK） | 再生の状態・位置・テンポが、前のサイクルの見こみと違うとき（`data->changed`）だけ。`frame`、`speed`、BBT が有効なら `barBeat`・`bar`・`beatUnit`・`beatsPerBar`・`beatsPerMinute`。`beat` は送らない | 0 | A [S16] |
| **Ardour** | サイクルの頭で、位置が飛んだ・速さが変わった・`time_scale` が変わった・拍の位置が 0.01 拍以上ずれた・テンポが変わった、のどれか。**さらに、サイクルの中の明示的なテンポ点ごとに**（`sample - start`）。欄は `frame`、`speed`、`barBeat`、`bar`、`beatUnit`、`beatsPerBar`、`beatsPerMinute`、`time:scale`。`beat` は送らない | 頭は 0、中は点の位置 | B [S12]（要約） |
| **JUCE の包み（受け手）** | 毎ブロックの頭で位置を空にし、そのブロックの列に `time:Position` があったときだけ作る。**ブロックに複数あれば最後の物が勝つ**。読む欄: `frame`、`speed`、`bar`、`beat`、`beatUnit`、`beatsPerBar`、`beatsPerMinute`。`barBeat` は読まない。対応: 拍子 = `beatsPerBar` と `beatUnit`、テンポ = `beatsPerMinute`、ppq = `beat`、再生中 = `speed` が 0 でない、小節数 = `bar`、時刻 = `frame` | | A [S9] |

- つまり、jalv や Ardour の送りかたでは、JUCE のプラグインは、変わったブロックだけ位置がわかり、ほかのブロックは位置なしになる（C、組み合わせからの推論）。Surge がそのとき前の値を覚えているかは、調べていない。

### 4-3. 受けるという宣言（A）
- 入力の atom ポートに `atom:bufferType atom:Sequence ; atom:supports time:Position`（Surge の例 [S8]。atom の説明にある、`midi:MidiEvent` の例と同じ形 [S4]）。jalv は、ポートが `time:Position` を受けるかで送りさきを決める [S16]。
- もう 1 つの道（約束事として使われている）: **control の入力ポートに `lv2:designation time:beatsPerMinute`**（や `time:speed`）。この PC の `gx_digital_delay`、`qmidiarp_*` がそれ（A [S20]）。Ardour は `bpm` をこのポートに毎サイクル入れる（B [S12]）。jalv も同じ（B [S16]）。

### 4-4. テンポがブロックの中で変わっていくとき
- 仕様は、テンポの「だんだん変わる」（傾き）を言い表さない。`time:Position` は、ある一点の位置と、その時の速さ（A [S6]）。
- Ardour は、テンポの点ごとに、ブロックの中の時刻つきで `time:Position` を足す（B [S12]）。jalv はしない（A [S16]）。JUCE の包みは、ブロックの中の複数を受けても、最後の 1 つだけを「そのブロックの位置」として使う（A [S9]）。

## 5. `ui:touch`（人がさわっている）

### 形（A [S7]。ui 2.24）
- `LV2UI_Touch { handle; void (*touch)(handle, uint32_t port_index, bool grabbed); }`。UI が `instantiate` の features で受け取る（URI `LV2_UI__touch` = `http://lv2plug.in/ns/extensions/ui#touch`）。
- 意味: 「UI のコントロールがつかまれた・放された事を、ホストに知らせる。**ホストは、放されるまで、そのポートを自動で動かすのをやめるか、値をいじらないべき**」。「モーター付きフェーダーをつかむのと同じ」。MIDI ラーンにも使える。
- 引数は**ポート番号**。patch のパラメータ（URI で指す物）を指す手段は、仕様にない。patch 2.10 に gesture・touch・grab の語はない（A [S5]）。

### Ardour（A/B [S14]）
- Suil に `suil_host_set_touch_func(ui_host, LV2PluginUI::touch)` で渡す。範囲外と、入力の control でないポートは無視。`grabbed` なら `control->start_touch(transport_sample)`、放されたら `stop_touch`。
- UI から control を書く `write_from_ui` は、**自分では touch を始めない**（touch は別の道）。
- Ardour 独自の「automate」拡張（`http://ardour.org/lv2/automate`: `start`、`end`、`parameter`、`value`、`control`、`controlled`、`canWriteAutomatation`）が別にある（A [S15]）。これは、プラグインの音の側が、自分の変化を線として書き込ませるため。人がさわる知らせとは別の物。

### patch のパラメータ（Surge XT など）
- **Surge XT の窓は、`ui:touch` を宣言していない**（ui.ttl の feature は `ui:idleInterface`、`urid:map`、`ui:parent`、instance-access、`ui:resize`、`opts:*` だけ。A [S8]）。JUCE の窓も、touch を一度も呼ばない。`beginChangeGesture`・`endChangeGesture` もない。JUCE のコードに、「`// TODO Implement begin/end change gesture support once it's supported by LV2`」とあり、`sendTouched` は何もしない（A [S9]）。
- 窓で動かした値は、DSP 側が次の `run()` の終わりに、**出力の atom ポートへ `patch:Set`（時刻 0）**を書いて知らせる（A [S9]）。ホストが入れた値は、はね返らない（3 章）。この「出力の `patch:Set`」は、窓が動かした印として使える。ただし、プラグインの中の別の理由で変わった値（プリセット読みこみなど）も同じ形で出うる（C、推論）。
- jalv は `ui:touch` を出さない、らしい（jalv.c の features に touch がない [S16]。プラグイン作者のコメントでも「jalv は何もしない」と読める [S21] C）。

## 6. ホストが値を変えたとき、窓に知らせる

- `LV2UI_Descriptor::port_event(ui, port_index, buffer_size, format, buffer)`（A [S7]）。
  - **形式 0**: control ポート。値が変わったときに呼ぶ（「変わるたび毎回でなくてもよい」）。`buffer_size` は `sizeof(float)`、`buffer` は float 1 つ。
  - **既定では、ホストが呼ぶのは control の入力ポートだけ**。ほかのポートは、UI が `ui:portNotification`（宣言で固定）か `ui:portSubscribe`（動いている最中に頼む）で頼む。`ui:notifyType` で、「この型だけ」に絞れる。
  - 形式 = `atom:eventTransfer` の URID: `buffer` は `LV2_Atom` 1 つ（時刻の頭なし）（A [S4]）。ホストは、イベントを 1 つずつ渡す。
  - `ui:floatProtocol`: 「値が変わったらできるだけ早く `port_event` を呼ぶべき。時間の保証はない」（A [S7]）。
- jalv: プラグイン → UI は輪（`plugin_to_ui`）で運ぶ。control の変化は `port_event`（形式 0）、atom は `eventTransfer`。UI 側は、`patch:Set` の物なら、property の URI で欄を探して値を直す（B [S16]）。
- Ardour: control は超速のタイマー（`output_update()`）で、前に送った値と違うときだけ `suil_instance_port_event`。atom は `emit_to_ui` で、`eventTransfer` の形式（B [S14] [S12]）。
- **ホストが入れた `patch:Set`（オートメーション）を窓へ知らせる決まりは、仕様に見つからなかった**。送るポート番号の決まりも見つからなかった。
- **Surge XT**: JUCE の窓の `portEvent` は空（「窓がプロセッサを直接聞いているので気にしない」）、DSP と窓は同じプロセスで instance-access を使う（A [S9] [S8]）。ホストから窓へ知らせる必要がない。窓が見ているプロセッサの値は、`setValueNotifyingHost` で変わり、窓の聞き手に伝わる（B、この道の中は読んでいない）。
- meloet の今: control のつまみは、見せた値と違えば `port_event(index, 0, float)`。出力の control も同じ。DSP が出した atom は `eventTransfer` で窓へ（A [S19]、`lv2.rs` の `show`）。ホストが入れた patch の値は、この道に乗っていない（A [S19]）。

## meloet への当てはめ（C、推論）

### control ポートのプラグイン
- **ブロックを分けるか**: オートメーションの点や、傾きを刻んだ点があるブロックだけ分けて、`run()` を何回かに分ける。分けた区間の頭で `set_control_input` する。プラグインが `coarseBlockLength` を宣言していたら（実物の例: nodelay）、分けずに、ブロックの頭で 1 回（Ardour と同じ）。
- 最小の長さは、Ardour に決まりがない。meloet は livi の最小が 1 なので 1 こままで技術的に可能だが、1 回ごとの呼び出しの重さがある。傾きを刻むなら 32〜64 こま（256 こまのブロックを 4〜8 つに）くらいから測って決める（数は C）。
- 時刻の `frame` は、分けた区間の頭を表すだけなので、control ポートに時刻の付け所はない。

### `patch:Set` のプラグイン（Surge XT などの JUCE の包み）
- **時刻つきで送っても、JUCE は見ない**。いまの形（1 ブロックに同じ property は最後の値だけ、フレーム付き）は、Surge では「ブロックの頭で当てる」と同じ。そのままで困らない。
- ブロックの中で値を変えたければ、分けて呼ぶしかないが、Surge は 1 回の `run()` が重い（20-2 の数）ので、分ける回数が増える費用が大きい。**1 ブロックに 1 つ（ブロックの頭）が、いちばん素直**（5.33 ms の間隔で値が変わる形）。
- 時刻つきで送る意味があるのは、時刻を守るプラグイン（あれば）。守るかは、プラグインごとに違うので、プラグインを見分ける手段がない限り、時刻 0（または分けた区間の頭）が無難。

### `time:Position`
- 受けるポート（`atom:supports time:Position`）があれば、**毎ブロック、時刻 0 に 1 つ**送る。JUCE の包みが毎ブロック要求するため（A [S9]）。jalv や Ardour の「変わった時だけ」は、JUCE では位置なしのブロックができる。
- 欄は全部: `frame`、`speed`、`bar`、`barBeat`、`beat`、`beatUnit`、`beatsPerBar`、`beatsPerMinute`。**`time:beat` を必ず入れる**（JUCE が ppq に使う。jalv や Ardour は送らない欄）。`beat` の数え方（`beatUnit` の音符 1 つが 1）と、JUCE が 4 分音符の数と読む事のずれに、注意。
- テンポがだんだん変わる所では、**ブロックの頭のテンポ**を毎ブロックの頭に入れれば、ブロックごとの階段で追える。JUCE は 1 ブロックに 1 つしか使わないので、ブロックの中に時刻つきで足しても意味がない（足すと、JUCE はその最後の物を「ブロックの頭の位置」として使うので、少しずれる）。分けて呼んだときは、区間の頭ごとに 1 つ。
- control ポートの `lv2:designation time:beatsPerMinute`・`time:speed` がある物（qmidiarp など）は、毎ブロック値を入れる（jalv・Ardour と同じ）。
- 拍子が 3/4 に変わるとき: `beatsPerBar` と `beatUnit` を変え、`bar`・`barBeat` を合わせる。変わる位置がブロックの中なら、分けるか、その次のブロックの頭で（C）。

### touch（人がさわっている）
- control ポート: `ui_touch` が既にある（A [S19]）。つかんでいる間は、オートメーションでそのポートを動かさない（仕様 [S7] の「ホストは動かすのをやめるべき」）。
- patch のパラメータ（Surge XT）: touch の知らせが来ないので、**窓から来た `patch:Set`（DSP が出力へ書く）を、人が動かした印**にして、「最後に動いてから 0.3〜0.5 秒ない」ことを touch の終わりと見なす案（数は C）。JUCE はホストが入れた値をはね返さないので、自分が送った値と取りちがえにくい（A [S9]）。ただし、窓を開いているときだけ（`ui_open`）に限り、プリセット読みこみの変化とは区別できない事に注意（C）。
- VST3 には `beginEdit`・`endEdit` があるので、LV2 は「推しはかり」になる事を、画面に出さなくてよいかは、棟梁が決める。

### 窓への知らせ
- control ポートは、今の道で足りる。Surge XT は、何もしなくてよい。ほかの patch プラグインの窓は、`patch:Set` を `atom:eventTransfer` で出すのが jalv の流れ。どのポート番号を使うかは、仕様に決まりがなく、調べきれていない（わからない）。

## わからなかったこと・追加で調べるとよいこと
- ホストが入れた `patch:Set` を窓へ知らせるときの、`port_index` の決まり（仕様に見つからず、jalv と Ardour の該当コードは読めなかった）。
- Carla の `process` の `time:Position` の送りかたと、分ける条件（ファイルが大きく、該当部分は読めなかった）。
- Surge の中身（JUCE の下の `SurgeSynthProcessor`）が、パラメータ変更と `getPosition()` が空のとき、どう動くか。
- 手もとの Surge XT のバイナリが、読んだ JUCE の版（`cf5754b`）でビルドされたか（確かめていない）。
- Ardour の `PluginPropertyAutomation`（patch のオートメーション）が、実際に 1 サイクル何回 `patch:Set` を送るか。
- 読んだ GitHub / GitLab のソースは、取りこみの道具が内容を要約して返したため、短い引用以外は要約を通している。重要な箇所は 2 回に分けて確かめた（JUCE の `run`、Ardour の分け方）。
- 仕様の版は、手もとの実物の版（core 18.6、atom 2.4、patch 2.10、time 1.6、ui 2.24、buf-size 1.4）。より新しい版は見ていない。

## 出どころ
| ID | URL または場所 | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | `/usr/lib/lv2/core.lv2/lv2core.meta.ttl`、`lv2core.ttl`、`manifest.ttl`（core 18.6）。https://lv2plug.in/ns/lv2core も同じ | control ポートは `run()` の間固定、control のレートはブロックの大きさ（ホストが決める）、CVPort | A | 2026-11-20 |
| S2 | `/usr/include/lv2/core/lv2.h` | `run` の説明、`sample_count` 0 の特別な場合 | A | 2026-11-20 |
| S3 | `/usr/lib/lv2/buf-size.lv2/buf-size.meta.ttl`、`manifest.ttl`（1.4） | `coarseBlockLength` ほか | A | 2026-11-20 |
| S4 | `/usr/lib/lv2/atom.lv2/atom.meta.ttl`、`/usr/include/lv2/atom/atom.h`、`forge.h`（atom 2.4）。https://lv2plug.in/ns/ext/atom | Sequence の時刻の単位、`atom:supports`、`eventTransfer` | A | 2026-11-20 |
| S5 | `/usr/lib/lv2/patch.lv2/patch.meta.ttl`（2.10）。https://lv2plug.in/ns/ext/patch | `patch:Set` ・ `Get`。gesture・時刻の決めはない | A | 2026-11-20 |
| S6 | `/usr/lib/lv2/time.lv2/time.ttl`、`time.meta.ttl`（1.6）。https://lv2plug.in/ns/ext/time | `time:Position` の全部の欄と型 | A | 2026-11-20 |
| S7 | `/usr/lib/lv2/ui.lv2/ui.meta.ttl`（2.24）、`/usr/include/lv2/ui/ui.h` | `LV2UI_Touch`、`port_event`、`write_function`、`portNotification`、`portSubscribe`、`floatProtocol` | A | 2026-11-20 |
| S8 | `/usr/lib/lv2/Surge XT.lv2/dsp.ttl`、`ui.ttl`（1.3.4） | `in` の `atom:supports midi, patch:Message, time:Position`、`rsz:minimumSize 63992`、UI の feature（touch なし） | A（実物） | 2026-11-20 |
| S9 | https://raw.githubusercontent.com/surge-synthesizer/JUCE/cf5754b19c87ea63758802e3f4239c05a77f1412/modules/juce_audio_plugin_client/juce_audio_plugin_client_LV2.cpp | `run()`（時刻無視、1 回の processBlock）、`PlayHead`、`setValueFromHost`、はね返りなし、touch なし、`portEvent` 空 | A（4 回読み、要約を通した） | 2026-11-20 |
| S10 | https://api.github.com/repos/surge-synthesizer/surge/contents/libs?ref=release_xt_1.3.4 | `libs/JUCE` の submodule が `cf5754b19c87…` | A | 2026-11-20 |
| S11 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/plugin_insert.cc | `automate_and_run` の分け方、最小の長さなし、VST3 は分けない | A（要約を通した） | 2026-11-20 |
| S12 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/lv2_plugin.cc | `time:Position` の送る条件と欄、テンポ点、`bpm` ポート、`coarseBlockLength` で分けない、`nframes - 1` | A（読めたのは前の 10 万字、要約を通した） | 2026-11-20 |
| S13 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/ardour/plugin.h、`lv2_plugin.h` | `requires_fixed_sized_buffers`、`_no_sample_accurate_ctrl`、`_bpm_control_port` | A | 2026-11-20 |
| S14 | https://raw.githubusercontent.com/Ardour/ardour/master/gtk2_ardour/lv2_plugin_ui.cc | `suil_host_set_touch_func`、`start_touch` ・ `stop_touch`、`output_update`、`write_from_ui` | A（要約を通した） | 2026-11-20 |
| S15 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/ardour/lv2_extensions.h | Ardour の automate 拡張ほか | A | 2026-11-20 |
| S16 | https://gitlab.com/drobilla/jalv/-/raw/main/src/jack.c、`src/jalv.c` | `time:Position` は変わった時だけ・時刻 0、`bpm` ポート、`plugin_to_ui` の輪、touch の宣言なし | A（要約を通した。`jalv.c` 以外の frontend は読んでいない） | 2026-11-20 |
| S17 | https://raw.githubusercontent.com/falkTX/Carla/main/source/backend/plugin/CarlaPluginLV2.cpp | control の変化を時刻で区切るらしい。`process` の `time:Position` は読めなかった | B（一部のみ） | 2026-11-20 |
| S18 | `~/.cargo/registry/src/index.crates.io-*/livi-0.9.0/src/features/mod.rs`、`plugin.rs` | 最小・最大のブロック長、最小より小さいと誤り | A | 2026-11-20 |
| S19 | `/srv/meloet-agent/meloet/crates/meloet-plughost/src/formats/lv2.rs`、`lv2/ui.rs`、`crates/meloet-plugin/src/ipc/mod.rs` | meloet のいまの作り（control は頭で入れる、patch は時刻つき・最後だけ、`ui_touch`、`port_event`、`Transport` の欄、`time:Position` は送っていない） | A（手もとのコード） | 2026-11-20 |
| S20 | `/usr/lib/lv2/nodelay.lv2/nodelay.ttl`、`qmidiarp_*.lv2`、`gx_digital_delay*.lv2` | `coarseBlockLength` の使い手、`lv2:designation time:beatsPerMinute ・ time:speed` | A（実物） | 2026-11-20 |
| S21 | https://docs.rs/crate/truce-lv2/0.58.1/source/src/ui.rs（検索結果の要約のみ） | 「ジェスチャーを使うのは Ardour と Reaper。jalv などは何もしない」という作者のコメント | C | 2026-11-20 |

2 年より古い情報: LV2 の仕様の文は安定しているので、A のまま。版の数だけは「手もとの版」と書いた。
