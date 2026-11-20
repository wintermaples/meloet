# 音のファイルを読む ・ 書き出す（mp3 ・ ogg ・ サンプルレート変換 ・ 書き出しのプラグインの作法）

調べた日 2026-11-21（meloet-research）。確かさは A・B・C。[S番号] は最後の表。symphonia は手もとの 0.6.1。LV2 の仕様は、この PC に入っている物（core 18.6、lv2core の manifest は minorVersion 18 ・ microVersion 6）。

## 結論（先に）

### 使える ・ 使えない ・ オーナーさんに聞く

| 部品 | 判定 | ひとこと |
|---|---|---|
| symphonia 0.6.1 の `mp3`（`symphonia-bundle-mp3`）、`ogg` + `vorbis` | **使える** | どれも MPL-2.0。ファイルを書きかえずに使うので決まりの内（A [S1] [S6] [S7]）。いま使っている `flac` ・ `wav` ・ `pcm` と同じ作り。feature 名は `mp3`（`mpa` は mp1 ・ mp2 ・ mp3 の全部入り）、`vorbis`、`ogg`（A [S1]） |
| opus を読む | **symphonia だけでは読めない** | symphonia 0.6.1 に opus の feature が無い。ogg の中の Opus を見つけて「頭の飛ばす数」を出すだけ（A [S1] [S3]、公式表も「作業中か未着手」 A [S4]、Issue #8 が開いたまま A [S8]）。足すなら純 Rust の `opus-decoder`（MIT OR Apache-2.0、libopus の移植、2026-03 公開、まだ新しい。B [S25] [S26]）。**いまは「opus は読まない」でよい**（C、推論） |
| rubato 5.1.0（サンプルレート変換） | **使える** | MIT OR Apache-2.0。依存も全部 MIT か MIT OR Apache-2.0（A [S10] [S11]）。音の質は FFT 版と sinc 版が上（A [S10] [S11]）。**曲を読むとき（音のスレッドの外）に全部変えてしまう使いかたを勧める**（C、推論） |
| wav を書く | **使える**（`hound` 3.5.1、Apache-2.0。または自分で書く） | hound は 2023-09 から更新なし（A [S13]）。wav の頭は短いので自分で書くのも安い（C、推論） |
| flac を書く | **使える**（`flacenc` 0.5.1、Apache-2.0、純 Rust） | 8〜24 ビット、8 チャンネルまで（A [S14] [S15]）。0.x でほかの版が出るかもしれない。libFLAC は BSD-3 風（Xiph）で C のライブラリなので、Rust のクレートで足りるうちは要らない（A [S16]） |
| mp3 を書く（LAME ・ `mp3lame-encoder`） | **使えない** | LGPL（`mp3lame-encoder` 0.3.0 は LGPL-3.0。A [S17]）。決まりで外す |
| mp3 を書く（`shine-rs`） | **使えない** | LGPL-2.0（A [S18]） |
| mp3 を書く（`rusty_mp3` 1.1.0） | **オーナーさんに聞く** | 純 Rust、Apache-2.0、エンコーダーあり（A [S19]）。**LGPL でない mp3 エンコーダーは、これしか見つからなかった**。ただし 2026-07-29 に出たばかりで 19 版、作りは自分で「仕様書から自前で作った」と言うだけ（B [S20]）、聞いた質の測りは無い。**「mp3 は出さない」でもよい**と仕様書にある。出すなら試験（ffmpeg で読み戻して比べる、聞く）をしてから |
| ディザー | 作法あり（B） | 24（または浮動小数）→ 16 ビットのときだけ、三角形（TPDF）、振れ幅 ±1 LSB（頭から頭まで 2 LSB）、書き出しの最後に 1 回（B [S27]、手順は C） |

### 書き出しで速く回すとき（先に 5 つ）

1. **VST3 のモードは 3 つ。名前は `kRealtime` ・ `kPrefetch` ・ `kOffline`**（依頼文の `kPrepare` ではない）。`kRealtime` ↔ `kPrefetch` は音のスレッドで、`setupProcessing` なしで切りかわる。**`kRealtime`（か `kPrefetch`）↔ `kOffline` は、ホストが `setupProcessing` を呼びなおして知らせる決まり**（A [S21]）。`setupProcessing` は「`setActive(true)` が呼ばれていない状態」で呼ぶ（A [S21]）ので、順番は `setProcessing(false)` → `setActive(false)` → `setupProcessing`（`processMode = kOffline`）→ `setActive(true)` → `setProcessing(true)`（B、ヘッダの 2 つの文と Ardour の `update_processor` から組んだ順。[S21] [S22] [S30]）。**meloet はいま `kRealtime` だけ**（A [S29]。`vst3.rs` の `setupProcessing` と `ProcessData` の 2 か所）。
2. **JUCE の VST3 の包みは、`setupProcessing` の `processMode` と、毎ブロックの `ProcessData::processMode` の両方から「offline か」を決める**（`setNonRealtime(processMode == kOffline)`。A [S31]）。**2 か所をそろえて送る**。Ardour も 2 か所に `kOffline` を入れる（A [S22]）。
3. **LV2 に「offline」の印は 1 つだけ: `lv2:freeWheeling` の control ポート**（0 か 1。「1 なら、できるだけ速く処理していて、実際の時間と処理している時間のあいだに関係が無い」A [S23]）。ほかに、plugin が自分で言う `lv2:isLive`（「ホストは実際の時間くらいの速さで `run()` を呼んでほしい」と頼む印）がある（A [S23]）。**この PC の LV2 には `isLive` を言うプラグインが 1 つも無い**（grep で 0 件。A [S28]）。`freeWheeling` ポートを持つのは Surge XT ・ Surge XT Effects ・ Carla の包みなど 27 ファイル（A [S28]）。**meloet はこのポートを「ホストが持つ物」として扱っている（書く値は確かめていない）**（A [S29]）。書き出しのときは 1 を書く。
4. **ほかの DAW は、速く回して壊れるプラグインを「見分けない」。人が選ぶ**: Ardour は書き出しの窓の「Realtime Export」（全体も、時間の区間ごとの RT も）で人が選ぶ。プラグインの一覧は持たない（A [S24] [S22]）。Bitwig も「Real-time」を選ぶ（A [S32]）。REAPER の公式の文は見つからなかった。
5. **meloet の形（別のプロセス）で一番気をつける所**: いまのエンジンは、プラグインの返事を「ブロックの長さ × 0.9」までしか待たず、遅れたらそのブロックを音なし（エフェクトは素通し）にする（A [S29]。`engine.rs` の `set_deadline`、`architecture.md` 20-2）。**書き出しでこれをやると、結果が回すたびに変わる**（C、推論）。書き出しでは「待つ」。上限は、いまの見はり（`HANG_MS` ＝ 5 秒）だけにする。

## 1. 音のファイルを読む

### 1-1. symphonia 0.6.1 の feature とライセンス（A）

- `Cargo.toml`（手もと [S1]）の feature: `mp3 = ["dep:symphonia-bundle-mp3", "symphonia-bundle-mp3/mp3"]`、`mpa = ["mp1","mp2","mp3"]`、`ogg = ["dep:symphonia-format-ogg"]`、`vorbis = ["dep:symphonia-codec-vorbis"]`、`opus` は**ない**。`default` には `vorbis` ・ `ogg` はあるが `mp3` は**ない**。
- 入れる形: `symphonia = { version = "=0.6.1", default-features = false, features = ["flac","wav","pcm","mp3","ogg","vorbis"] }`。試作 `spikes/sample-load/Cargo.toml` はもう `flac ・ vorbis ・ ogg` で動かしている（A [S29]）。
- ライセンス: `symphonia`（本体）、`symphonia-core`、`symphonia-metadata`、`symphonia-bundle-flac`、`symphonia-codec-pcm`、`symphonia-codec-vorbis`、`symphonia-format-ogg`、`symphonia-format-riff` は、手もとの `Cargo.toml` で **全部 `MPL-2.0`**（A [S1] [S6]）。ogg の `LICENSE` の頭は「Mozilla Public License Version 2.0」（A [S6]）。`symphonia-bundle-mp3` 0.6.1 も crates.io で `MPL-2.0`（A [S7]）。mp3 のクレートは手もとの `~/.cargo/registry` にまだ無い（入れたことがない）。入れたら `LICENSE` ファイルを見なおす。
- **MPL-2.0 は「ファイル単位」**。symphonia のファイルを書きかえずに依存で使うだけなら、決まりの内（決まりの文: MPL-2.0 の部品はファイルを書きかえない）。`node licenses/check.mjs` に mp3 のクレートが増えるので、見はりを通す。

### 1-2. mp3 の質と gapless（頭の無音）

- 公式の表（master の README [S4]）: **MP3 ・ Vorbis とも状態「Excellent」、gapless「Yes」**。「gapless は demuxer と decoder の両方の助けが要る」と書いてある。
- 0.6.1 の更新（A [S5]）: 「MP3: 一定の条件で、無音の区間が読まれないバグを直した」「Xing/Info と VBRI のタグが、CRC つきのフレームのとき無視されていたのを直した」。0.6.0 と 0.6.1 に、gapless の変更は無い。0.6.1 より新しい版は無い（2026-11-21 に見た）。
- **mp3 の gapless のしくみ**（v0.6.1 の `demuxer.rs`。A [S9]。要約を通して読んだ）:
  - 最初のフレームが Xing / Info / VBRI なら、それを食べて音には使わない。
  - 頭と終わりの無音の数は、**Xing/Info の LAME の拡張欄**からだけ。**作った物の ID が `LAME` ・ `Lavf` ・ `Lavc` で始まるときだけ**読む。ほかは `(0, 0)`。式は `delay = 528 + 1 + (trim >> 12)`（デコーダーの遅れ 529 を足している）。CRC が合わなければ捨てる。
  - 遅れのぶん、時刻を負から始める。`num_frames`（長さ）は遅れと終わりの余りを引いた数。
  - タグが無い mp3 は、長さを「平均のフレームの大きさ」から見積もる（VBR だとずれる、と自分でログに出す）。
  - **だから、LAME / ffmpeg 以外で作った mp3 や、タグの無い mp3 は、頭に符号器の遅れとデコーダーの遅れ（LAME なら合わせて 1105 こま前後。数は B [S36]）の無音が残る**（C、推論。コードの「(0,0)」から）。44.1 kHz なら約 25 ms。ピアノロールの拍にのせる素材では、耳でわかるずれ。
- ogg（Vorbis）の gapless: `symphonia-format-ogg` が、最初と最後のページの位置から「頭の遅れ」と「終わりの余り」を出し、パケットの `trim_start` ・ `trim_end` にする（A [S3]、`logical.rs`）。`symphonia-codec-vorbis` は `gapless` が真のときその数を切る（A [S3]、`lib.rs` 317〜324 行）。`AudioDecoderOptions::default()` は `gapless: true`（A [S2]）。**いまの meloet の読みこみ（`sampler/load.rs`）は `AudioDecoderOptions::default()` なので、そのまま gapless になる**（A [S29]）。
- 0.6 系で見つかった問題: 開いている Issue に mp3 のラベルは 0 件（A [S8]、2026-11-21 に見た）。0.5.4 の「シークのあと mp3 にノイズ」Issue #274 は 0.6 より前の話（B [S33]）。opus 関連の開いている Issue: #8「Opus の状態」、#572「Ogg の `Packet::dts` が 0 のまま」、#576 など（A [S8]）。**0.6 の mp3 や ogg の gapless に、直っていない大きな問題は見つからなかった**（C。見つからないことの確認で、無いとは言えない）。
- 質の測り（ffmpeg と比べた数）は一次情報で見つからなかった。**本番の前に、LAME と ffmpeg で作った mp3 と ogg を読んで、長さと頭の位置を測る試験を勧める**（C）。

### 1-3. opus（A / B）

- symphonia 0.6.1: `symphonia-format-ogg` に Opus の入れ物の読み（`OpusHead` の `pre_skip` を `with_delay` に入れる）はあるが、**デコーダーが無い**（A [S3] [S1]）。公式の表の Opus は「-」（作業中か未着手。A [S4]）。
- 純 Rust のデコーダー: `opus-decoder` 0.1.1（2026-03-12、`MIT OR Apache-2.0`、「libopus の移植」、RFC 8251 の 12 本の試験が全部通る、と README が言う。ファジングは未、速さは未。Ogg の読みは入っていない）（B [S25] [S26]）。symphonia の ogg が出すパケットを渡す形は使えそう（C、推論）。移植元は libopus（BSD-3）なので、帰属表示を足す必要があるかもしれない（C）。**第 7 回に opus は要らない、と勧める**（C）。

## 2. サンプルレートを変える（曲が 48000 Hz、ファイルが 44100 Hz など）

### rubato 5.1.0（A）

- ライセンス `MIT OR Apache-2.0`（crates.io の 5.1.0。リポジトリの README も同じ）。2026-11-12 に 5.1.0。Rust 1.87 以上が要る（A [S10] [S11]。meloet の道具の版は見ていない）。
- 依存（A [S12]）: `audioadapter` 5.0.0（MIT OR Apache-2.0）、`audioadapter-buffers` 5.2.0（MIT OR Apache-2.0）、`num-integer` ・ `num-traits`、`windowfunctions` 0.1.1（**MIT**）、`visibility`、任意で `realfft` 3.5.1（**MIT**、`fft_resampler` が既定で有効）・ `num-complex`・ `log`。`realfft` は `rustfft` 6.4.1（MIT OR Apache-2.0）に依る。**どれも GPL ・ LGPL ではない**（`num-integer` ・ `num-traits` ・ `visibility` ・ `num-complex` の値は crates.io で見ていない。広く MIT OR Apache-2.0 だと記憶しているが記憶（C））。
- 種類（A [S10]）:
  - `Fft`（同期）: 比が決まっているとき（44.1 → 48 kHz）。「速くて質が高い」が、比は変えられない。**窓は BlackmanHarris2、約 256 こまごとの小さな FFT**。遅れは「FFT の大きさの半分」（出力のこま数。`output_delay()` で聞く。A [S11]）。
  - `Async::new_sinc`: 帯域を制限した sinc。質は `Fft` と同じくらい。いちばん CPU を使う。比を後から変えられる（`as_adjustable()`）。
  - `Async::new_poly`: 多項式だけ。速いが、折り返し防止のフィルターが無い。
- **その場で鳴らすとき（音のスレッド）**: README は「realtime で安全に作った」と言う（A [S10]）。`process_into_buffer()` は、**作っておいた出力のバッファに書き、メモリを取りにいかない**（docs.rs の文: `process` は毎回メモリを取るので「リアルタイムなら `process_into_buffer`」。A [S11]）。ただし `Fft::new` ・ `Async::new_*` や、バッファの用意は、音のスレッドの外でやる。`log` feature は切ったまま。
- **あとから全部変える**: `process_all_into_buffer`（作った出力の長さは `process_all_needed_output_len` で先に聞く）。**初めの無音（遅れのぶん）を切って**書く（A [S11]）。曲を読むとき、`symphonia` が出した `Vec<f32>` をこれで 48 kHz にしてサンプラーに渡せば、音のスレッドは今のまま（C、推論）。ピッチやテンポで比が連続して変わるとき（可変速）だけ、`Async` を音のスレッドで使う。

## 3. 書き出し

### 3-1. wav ・ flac ・ mp3 を書く部品

| 形式 | 部品 | ライセンス | 確かさ | ひとこと |
|---|---|---|---|---|
| wav | `hound` 3.5.1 | Apache-2.0 | A [S13] | `WavWriter::new` は `Write + Seek` が要る。`finalize()` で頭を直す。最後の更新 2023-09-25（2 年と少し前。「止まっているかも」と書く）。16 ・ 24 ・ 32 ビット整数と 32 ビット浮動小数の対応は、この調べでは docs で確かめられなかった（C） |
| flac | `flacenc` 0.5.1 | Apache-2.0 | A [S14] [S15] | 純 Rust。8〜24 ビット、最大 8 チャンネル（`MIN/MAX_BITS_PER_SAMPLE` = 8/24、`MAX_CHANNELS` = 8。A [S15]）。入力は `i32` の列。`par` 既定で、複数スレッド（切れる）。`decode` は試験的。依存の `crc` 3.4.0 ・ `heapless` 0.9.4 ・ `seq-macro` 0.3.6 ・ `md-5` ・ `rustversion` ・ `num-traits` ・ `built`（作るとき）。前 3 つは MIT OR Apache-2.0（A [S12]）。ほかは見ていない（C） |
| flac（C のライブラリ） | libFLAC | BSD 風（Xiph） | A [S16] | 「ライブラリ（libFLAC ・ libFLAC++）は Xiph.org の BSD 風のライセンス。ほかのプログラムとプラグインは GPL」。README の頭の行は LGPL とも書く（どれがかは書いていない）。**クレートで足りるので勧めない**（C） |
| mp3 | LAME / `mp3lame-encoder` 0.3.0 | LGPL-3.0 | A [S17] | 決まりで外す |
| mp3 | `shine-rs` 0.1.5 | LGPL-2.0（shine の移植） | A [S18] | 外す |
| mp3 | **`rusty_mp3` 1.1.0** | **Apache-2.0** | A（ライセンス）[S19]、B（中身）[S20] | 純 Rust、CBR と VBR、ジョイントステレオ、聴感モデル、2 重ループの量子化。作った人は「Rust の mp3 エンコーダーは LAME の包みばかり」「Symphonia は MPL-2.0 で『コピーレフト無し』の見はりに引っかかる」「mp3 の特許は 2017 年に切れた」と書く。**母体は `Remade-With-Rust/remade_ffmpeg_rs`（Mata Network の「C ・ C++ の道具を Rust で作りなおす」）。「FFmpeg の再実装であって、FFmpeg のコードは使っていない」と言う（B [S34]）。リポジトリに `.claude` と `CLAUDE.md` があるので、AI を使って作ったらしい（C、推論）**。2026-07-29 に 0.1.0、19 版、1.0.0 は 2026-10-04。聞いた質の測り・長い試験は見つからない。デコーダーの「FFmpeg とビットまで同じ」は自分の言葉 |
| mp3 | `mpeg-audio` 0.1.1（MIT OR Apache-2.0） | | B（検索の要約のみ） | **エンコーダーではない**（できあがった mp3 のフレームを包むだけ） |

- **mp3 の特許**: Fraunhofer の公式のページに「Technicolor と Fraunhofer IIS の mp3 の特許のライセンス事業は 2017-04-23 に終わった」（最後の特許が切れたため）とある。ただし「個別の実装に関わる特許が残っているかもしれない」とも書く（A [S35]）。
- **結論（mp3）**: LGPL でない mp3 のエンコーダーは `rusty_mp3` の 1 つだけ見つかった。使うかはオーナーさんが決める。選べる道: (a) 出さない（仕様書で許されている）、(b) `rusty_mp3` を試験つきで入れる、(c) 使う人が自分で入れた `lame` の実行ファイルを別のプロセスで呼ぶ（リンクしないので LGPL にふれない。ただし「meloet だけで完結しない」。C、推論）。

### 3-2. ディザー（24 → 16 ビット）

- **三角形（TPDF）のディザー**: 「さらに処理する物には、量子化の 2 段分の幅の三角形のディザー」「ノイズの変調を起こさない」「RPDF（一様）を 2 つ足すと作れる」（Wikipedia の「Dither」、元は Vanderkooy & Lipshitz 1987 JAES 35(12)、Lipshitz ほか 1991 JAES 39(11)）（B [S27]。JAES の本文は読んでいない）。
- 作法（B の文から組んだ。手順は C）:
  1. 書き出す直前の 1 回だけ（ゲイン ・ レート変換 ・ マスターのメーターのあと）。
  2. 16 ビットへ落とすときだけ。24 ビット整数 ・ 32 ビット浮動小数 ・ flac 24 ビットには要らない（C）。
  3. 値 = `round(x * 32767 + (r1 + r2 − 1))`、`r1` `r2` は `[0, 1)` の一様な乱数。チャンネルごとに別の乱数（C）。
  4. 先に `[-1, 1]` で止める（うまく止めないと 32767 をこえる。C）。
  5. 噪音整形（ノイズシェイピング）は付けない。TPDF だけで足りる（C）。Wikipedia は、最後の出力では噪音整形もよい、とだけ書く（B [S27]）。
- mp3 エンコーダーへは、浮動小数のままかそれに近い形で渡す（ディザーを先に入れるとエンコーダーが 2 度目の量子化をする）。C、推論。

## 4. 実時間より速く回すときのプラグインの作法

### 4-1. VST3（A [S21]。`ivstaudioprocessor.h`、Steinberg の `vst3_pluginterfaces` の master）

`enum ProcessModes` の文（そのまま）:

- `kRealtime`: process の呼びは「実時間の頻度（`numSamples / samplerate`）」。プラグインは「他のプラグインに時間を残すため、できるだけ速く処理する」。
- `kPrefetch`: 呼びは「変わる頻度（ゆれ ・ 実時間より遅い ・ 速い）」。実時間と同じ質で処理する。「実時間まで遅くしてはいけない（例: ディスクのストリーミング）」。ホストは、サンプラー系のプラグインを `kPrefetch` で回すのを避けること。
- `kOffline`: 「実時間より速いか遅い。実時間より高い質を使ってよい」。ディスクのストリーミングを持つプラグインは、必要なら実時間以下に遅くしてでも、process の中でストリーミングが間に合うようにする。
- モードを切りかえる決まり: 「`kRealtime` ↔ `kPrefetch` は、`setupProcessing` なしで、リアルタイムのスレッドで切りかわる。プラグインは process ごとに `ProcessData::processMode` を見る。**`kRealtime`（か `kPrefetch`）と `kOffline` のあいだは、ホストが `IAudioProcessor::setupProcessing` を呼んで知らせる**」。

そのほか（A [S21]）:
- `ProcessSetup { processMode, symbolicSampleSize, maxSamplesPerBlock, sampleRate }` と、`ProcessData` の `processMode`（「値は ProcessModes」）。**どちらにもモードがある**。
- `setupProcessing`: 「無効の状態（`setActive` を `true` で呼んでいない）で、`setProcessing` の前に呼ぶ」。スレッドの注は `[UI-thread & (Initialized | Connected)]`。
- `setProcessing(state)`: 「`true` は process が始まる前、`false` は終わったあと」。「`setProcessing(false)` が、process なしで `setProcessing(true)` のあとに来てもよい」。「UI でも処理のスレッドでも呼ばれるので、軽い操作だけ（メモリを取らない、大きな再設定をしない）。遅れ線やリバーブのバッファのリセットに使える」。「`setActive(true)` のあいだだけ呼ぶ」。
- `getTailSamples`: 「ホストが offline の処理 ・ 処理の最適化 ・ ダウンミックスに使える」。戻り値は `kNoTail`（0）、秒 × サンプルレート、`kInfiniteTail`（`kMaxInt32u`）。
- `IProcessContextRequirements`: 「実装しないと、process の `ProcessContext` の情報が何も来ないかもしれない」。欲しい物の印（`kNeedTempo` など）を返す。
- `ProcessContext`: `projectTimeSamples` は常に有効。`systemTime`（ナノ秒、任意）は `kSystemTimeValid`、`continousTimeSamples`（ループを除いた時間）は `kContTimeValid`。
- **手順**（B、上の文と Ardour から組んだ）: `setProcessing(false)` → `setActive(false)` → `setupProcessing({kOffline, 同じ maxSamplesPerBlock と sampleRate})` → `setActive(true)` → `setProcessing(true)`。meloet は `reactivate()`（`kLatencyChanged` のとき）で、すでに「`setProcessing(0)`、`setActive(0)`、`setActive(1)`、`setProcessing(1)`」と回す形を持つ（A [S29]、`vst3.rs` 1021〜1033 行）。そこに `setupProcessing` を足す形で作れる（C、推論）。注: `setupProcessing` のスレッド注は `Initialized | Connected` だが、ワークフロー図は `Setup Done` からの再呼びを禁じていない（A [S21] [S37]。食いちがいではなく、書き方の違い）。Ardour は同じ順で呼んでいる（A [S22]）。
- 書き出しが終わったら、`kRealtime` に戻して同じ手順を踏む（C、推論）。

### 4-2. LV2（A [S23] [S38]）

- **offline を伝える物は `lv2:freeWheeling` の control ポートだけ**（`lv2:designation lv2:freeWheeling`）。仕様の文（`lv2core.meta.ttl`）: 「真なら、処理はできるだけ速く行われていて、実時間ではない。free-wheeling のとき、実際の時計の時間と、処理しているデータの時間のあいだに関係は無い」。ポートは任意（`lv2:connectionOptional`）。
- **worker**（`worker.h` の文）: `schedule_work` は「free-wheeling のとき（例: offline の書き出し）は worker をその場で動かしてよい。1 本のスレッドで、サンプル単位で、`run()` が実時間より速くても遅くても時間の問題が出ない」。プラグインは「worker がその場で動き、返事もその場で届いても、効きめがすぐ、サンプル単位で出る」ように書くべき（SHOULD）。つまり、**ホストは free-wheeling のとき、worker をその場で動かして返事をすぐ返す方がよい**（A [S38]）。meloet の LV2 の worker の扱いは見ていない（宿題）。
- **`lv2:isLive`**: 「時間に敏感な中身（例: ソケットで実時間に話す）を持つプラグインの印。入出力をためたり大きく遅らせたりしてはいけない、`run()` は `sample_count` で見て実際の時間くらいの速さで呼ぶこと」（A [S23]）。`hardRTCapable` とは別（A [S23]）。この印があるプラグインは、書き出しで速く回してはいけない（A 文 + C 推論）。**この PC の LV2 に `isLive` は 0 件**（A [S28]）。
- JUCE の LV2 の包み: `run()` のたびに `processor->setNonRealtime(freeWheeling ポートの値 > 0.5)`。ポートの `lv2:default 0.0`、`connectionOptional`（A [S39]。JUCE の develop）。Surge XT の LV2 はこのポートを持つ（A [S28]）。
- Ardour: `lv2:freeWheeling` のポートを探し、**サイクルごとに `_non_realtime || engine.freewheeling() ? 1.0 : 0.0` を書く**（A [S22]）。ポートはオートメーションの欄に出さない（A [S22]）。
- LV2 には VST3 の `getTailSamples` のような尾の長さを言う仕組みが、手もとの core には見つからなかった（A、見つからないことの確認 [S23]）。尾は「音が消えるまで、または決めた長さまで回す」しかない（C、推論）。

### 4-3. ほかの DAW のやりかた

| DAW | 速く回すときの作法 | 確かさ |
|---|---|---|
| **Ardour** | 書き出しは既定で「freewheel」（`engine.freewheel(true)`）。書き出しの窓の「Realtime Export」（全体）と各区間の「RT」にチェックすると、freewheel せず実時間で回す。manual は「一部のプラグイン（リバーブなど）のおかしな動きを避けられる」とだけ言う。**壊れるプラグインの一覧は持たない**（見つからなかった）。freewheel のとき: VST3 は process のたびに `kOffline`、LV2 は freeWheeling ポートに 1。実時間の書き出しは 1 秒の pre-roll、freewheel は設定の長さ。遅れを流しきる pre-roll の空回し（`worst_latency_preroll_buffer_size_ceil`）をする。終わりに `flush_all_inserts()`（freewheel のとき）。`freewheel(true)` の呼びは、書き出しの開始のところだけ | A [S24] [S22] [S40] |
| **VST3 の `kOffline` を Ardour は** | `update_processor()` で `setupProcessing` のときに `_process_offline || freewheeling ? kOffline : kRealtime`。**ただし `update_processor()` が呼ばれるのは「ブロックの大きさが変わったとき」と「`set_non_realtime` が変わったとき」だけ。freewheel の切りかえでは呼ばれない**。process のたびの `ProcessData::processMode` は毎回計算する。つまり `ProcessSetup` のモードは「その時の freewheel」で、あとから変わらないことがある（読んだ範囲 100000 文字まで。最後の 5779 文字は見ていない） | A [S22] |
| **Bitwig** | 書き出しの「Real-time」で、「標準の offline の書き出し」を実時間にできる。使いどころは「外の機器を実時間で通すとき」。プラグインへの言及は無い | A [S32] |
| **REAPER** | 公式の文は見つからなかった。wiki に「実時間の速さに制限」のチェックがあるが、くわしい文は取れなかった。JUCE の開発者の投稿: 「プラグインに offline を知らせる設定が REAPER にあり、既定では入っていない」（設定の名前 ・ 場所は書いていない） | C [S41] |
| **Ableton Live** | JUCE の開発者の報告: VST3 で offline に書き出しても `processMode` が `kRealtime` のまま（2021 年から 2024-07 まで同じ）。出力の機器と同じレートなら `setupProcessing` も呼ばない | B（開発者本人の報告） [S42] |
| **Cubase ほか** | フォーラムの話: 外部のエフェクトを使うと Cubase は実時間の書き出しを強制する。Cantabile の文書は「offline の書き出しは、機器に結ばれたプラグインや、実時間より速く動けないプラグインとは合わない」と言う | C / B [S43] |

- まとめ（B / C）: **見分ける自動の仕組みは、どの DAW にも見つからなかった**。「人が実時間の書き出しを選ぶ」と、「プラグインの側が offline を見て、自分で直す」の 2 つの道だけ。
- どんなプラグインが壊れるか（B / C、フォーラムの話）: ディスクのストリーミングのサンプラー、時計で動く LFO やスムージング、別のスレッドで読みこむ物、実時間の外の物（`isLive`）、offline を見て質を上げる物（oversampling 等）。**書き出しのときに「ブロックごとに結果が同じ」ことを試験（実時間と速い回しで音を比べる）で確かめるのが現実的**（C、推論）。

### 4-4. ホストとプラグインのプロセスが分かれているとき（meloet の形）

手もとのコード（A [S29] `architecture.md` 20 章、`vst3.rs`、`engine.rs`）と、上の調べからの推論（C）。

1. **モードを渡す道が要る**: 共有メモリの「枠」に、1 ブロックごとの `processMode` と「速く回す」印を入れる。`setupProcessing` の切りかえは、子の「主スレッド」（`control.rs`）でやる。音のスレッドは触らない（`reactivate()` が `gate.claim` を取って、その間音のスレッドを外す形の前例）。LV2 は、`freeWheeling` ポートの値を 1 ブロックごとに書けばよい（音のスレッドのまま、メモリは取らない）。
2. **書き出しの待ち方**: 通常の再生は「ブロックの長さ × 0.9 までしか待たず、ブロックを音なしにする」。書き出しでは、**待ちつづけ、音なしにしない**。上限は `HANG_MS`（5 秒）の見はり。止まったら書き出し自体を「失敗」にして人に伝える。「1 ブロックだけ音なし」にしない。
3. **遅れ（latency）**: プラグインの遅れの合計ぶん、頭の空回しをして切る（Ardour の pre-roll と同じ）。尾は、VST3 は `getTailSamples`、LV2 は無いので「音が小さくなるまで、決めた長さまで」。
4. **ブロックの大きさ**: `setupProcessing` の `maxSamplesPerBlock` はこえない。大きなブロックで速く回したくても、`setupProcessing` をやりなおす必要がある（JUCE などは `prepareToPlay` が走る）。**再生と同じ大きさのブロックで回すのが安全**（C）。
5. **窓**: 書き出し中も、プラグインの窓の別のスレッド（X11）は動いたまま。パラメータの変更が書き出しに混ざらないよう、書き出し中は窓の操作を止めるかを決める（宿題。C）。
6. **子が複数のとき**: 同じプラグインの枠は 1 つのプロセスにまとまる（20-1）ので、1 回の `setupProcessing` の切りかえは「そのプロセスの全部の枠」を一度に止めてから回す形になる（C、推論）。

## 5. 調べきれなかったこと ・ 追加で調べるとよいこと

- **`rusty_mp3` の質**: 1 度 mp3 を作って LAME と ffmpeg で読み戻し、長さ ・ 頭の位置 ・ 聴感を比べる。出所（再実装と言うが、ライセンスの来歴が確かめにくい）を、オーナーさんが納得するか。
- **mp3 の読みの質**: symphonia-bundle-mp3 の出力が、ffmpeg とどのくらい同じか（測りは見つからなかった）。LAME ・ ffmpeg ・ iTunes で作った mp3 で、頭のずれを測る。
- **REAPER ・ Studio One ・ Cubase の「offline の知らせ方」**: 公式の文は見つからなかった。
- **JUCE の LV2 ・ VST3 の包みが入った版**: この PC の Surge XT の JUCE の版が、`develop` の内容と同じかは見ていない（前の書類 `automation-lv2.md` の [S9] の版は 1.3.4 の submodule）。
- **LV2 の worker を meloet がどう呼ぶか**、`lv2:freeWheeling` に何を書いているか（`lv2.rs` 500〜520 行あたり）は読んでいない。
- **`flacenc` の 24 ビットの実際の動き**（上限が 24 なのは定数でわかる。入力が `i32` の符号つきで範囲内、と README が言う）、**`hound` の 24 ビット書き出し**は、docs では確かめられなかった（試験で見る）。
- `opus-decoder` の来歴（libopus の移植で BSD-3 の帰属が要るか）。

## 出どころ

| 番号 | 出どころ | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | 手もと `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/symphonia-0.6.1/Cargo.toml` | feature の名前（`mp3` ・ `mpa` ・ `ogg` ・ `vorbis`、opus 無し）、`license = "MPL-2.0"` | A | 2026-11-21 |
| S2 | 手もと `symphonia-core-0.6.1/src/codecs/audio.rs` 211〜236 行 | `gapless` が既定で `true`。「真なら遅れと余りのこまを切る」 | A | 2026-11-21 |
| S3 | 手もと `symphonia-format-ogg-0.6.1/src/logical.rs` ・ `mappings/opus.rs`、`symphonia-codec-vorbis-0.6.1/src/lib.rs` | ogg の頭と終わりの切り、Opus の `pre_skip` を `with_delay` に、Vorbis が `trim` を切る | A | 2026-11-21 |
| S4 | https://raw.githubusercontent.com/pdeljanov/Symphonia/master/README.md | デコーダーの表（MP3 / Vorbis は Excellent ・ gapless あり、Opus は「-」）、MPL-2.0。master（0.6.1 と同じとは限らない） | A | 2026-11-21 |
| S5 | https://github.com/pdeljanov/Symphonia/releases | 0.6.0 / 0.6.1 の変更（MP3 の無音の区間、Xing と CRC）。0.6.1 が最新 | A | 2026-11-21 |
| S6 | 手もと `symphonia-*-0.6.1/Cargo.toml`、`symphonia-format-ogg-0.6.1/LICENSE` | 全部 MPL-2.0 | A | 2026-11-21 |
| S7 | https://crates.io/api/v1/crates/symphonia-bundle-mp3 | 0.6.1 `MPL-2.0`、2026-08-13 | A | 2026-11-21 |
| S8 | https://github.com/pdeljanov/Symphonia/issues（絞りこみ） | mp3 ラベルの開いた Issue 0 件。Opus #8、#572、#576 | A | 2026-11-21 |
| S9 | https://raw.githubusercontent.com/pdeljanov/Symphonia/v0.6.1/symphonia-bundle-mp3/src/demuxer.rs | Xing / Info / VBRI の読み、LAME の遅れと余り、`528+1+…`、長さの見積もり（要約を通して読んだ） | A | 2026-11-21 |
| S10 | https://raw.githubusercontent.com/HEnquist/rubato/master/README.md | 種類、リアルタイムの作法、`MIT OR Apache-2.0`、Rust 1.87 | A | 2026-11-21 |
| S11 | https://docs.rs/rubato/5.1.0/rubato/trait.Resampler.html ・ `…/struct.Fft.html` | `process_into_buffer`、`process_all_into_buffer`（初めの無音を切る）、`output_delay`、`Fft::new` | A | 2026-11-21 |
| S12 | https://crates.io/api/v1/crates/{rubato,audioadapter,audioadapter-buffers,windowfunctions,realfft,rustfft,heapless,crc,seq-macro}（と `…/rubato/5.1.0/dependencies`、`…/flacenc/0.5.1/dependencies`） | 版とライセンス（rubato 5.1.0 MIT OR Apache-2.0、windowfunctions 0.1.1 MIT、realfft 3.5.1 MIT など） | A | 2026-11-21 |
| S13 | https://crates.io/api/v1/crates/hound ・ https://docs.rs/hound/3.5.1/hound/struct.WavWriter.html | 3.5.1、Apache-2.0、2023-09-25、`Write + Seek` が要る、`finalize` | A | 2026-11-21 |
| S14 | https://crates.io/api/v1/crates/flacenc ・ https://raw.githubusercontent.com/yotarok/flacenc-rs/main/README.md | 0.5.1、Apache-2.0、2025-12-18、純 Rust、`par` | A | 2026-11-21 |
| S15 | https://raw.githubusercontent.com/yotarok/flacenc-rs/main/src/constant.rs | `MIN_BITS_PER_SAMPLE`=8、`MAX_BITS_PER_SAMPLE`=24、`MAX_CHANNELS`=8 | A | 2026-11-21 |
| S16 | https://raw.githubusercontent.com/xiph/flac/master/README.md ・ `COPYING.Xiph` | libFLAC は Xiph の BSD 風。ほかのプログラムは GPL | A | 2026-11-21 |
| S17 | https://crates.io/api/v1/crates/mp3lame-encoder | 0.3.0、LGPL-3.0、LAME の包み | A | 2026-11-21 |
| S18 | https://crates.io/api/v1/crates/shine-rs | 0.1.5、LGPL-2.0、shine の移植 | A | 2026-11-21 |
| S19 | https://crates.io/api/v1/crates/rusty_mp3 | 1.1.0、Apache-2.0、純 Rust の mp3 のデコード ・ エンコード、2026-07-29 初出、19 版 | A（ライセンスと版） | 2026-11-21 |
| S20 | https://docs.rs/rusty_mp3 | CBR/VBR、聴感モデル、「LAME の包みばかり」「仕様書から自前」「デコーダーは FFmpeg とビットまで同じ」（自分の言葉） | B（作った本人の発言） | 2026-11-21 |
| S21 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstaudioprocessor.h ・ `ivstprocesscontext.h` | `ProcessModes`（`kRealtime` ・ `kPrefetch` ・ `kOffline`）の文、切りかえの決まり、`setupProcessing` ・ `setProcessing` ・ `getTailSamples`、`IProcessContextRequirements` | A | 2026-11-21 |
| S22 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/vst3_plugin.cc ・ `lv2_plugin.cc`（読んだ範囲は先頭の約 10 万文字） | `processMode = _process_offline \|\| freewheeling ? kOffline : kRealtime`（`ProcessSetup` と `ProcessData`）、`update_processor` の呼ばれる時、LV2 の `freeWheeling` ポートへの書きこみ | A（要約を通して読んだ） | 2026-11-21 |
| S23 | 手もと `/usr/lib/lv2/core.lv2/lv2core.ttl` ・ `lv2core.meta.ttl`（`freeWheeling` 444〜452 行、`isLive` 555〜567 行）、`/usr/include/lv2/core/lv2.h` | `freeWheeling` と `isLive` と `hardRTCapable` の仕様の文 | A | 2026-11-21 |
| S24 | https://manual.ardour.org/exporting/export-dialog/ | 「Realtime Export」と区間ごとの「RT」、「一部のプラグインの奇妙な動きを避ける」 | A | 2026-11-21 |
| S25 | https://crates.io/api/v1/crates/opus-decoder | 0.1.1、MIT OR Apache-2.0、2026-03-12 | A（版と値） | 2026-11-21 |
| S26 | https://docs.rs/crate/opus-decoder/latest/source/README.md | 純 Rust ・ libopus の移植、RFC 8251 の 12 本通る、ファジング未、速さは未、Ogg は別 | B（作った本人の発言） | 2026-11-21 |
| S27 | https://en.wikipedia.org/wiki/Dither | TPDF は 2 段分の幅、ノイズの変調なし、RPDF を 2 つ足す。元の論文（JAES 1987 と 1991）は読んでいない | B | 2026-11-21 |
| S28 | 手もと `/usr/lib/lv2/**/*.ttl` の検索（`isLive` 0 件、`freeWheeling` 27 ファイル） | この PC のプラグイン | A（手もと） | 2026-11-21 |
| S29 | 手もと `/srv/meloet-agent/meloet/`: `crates/meloet-plughost/src/formats/vst3.rs`（491 ・ 792 ・ 1021〜1033 行）、`lv2.rs`（228 ・ 511 行）、`crates/meloet-engine/src/engine.rs`（1264 行）、`crates/meloet-sound/src/sampler/load.rs`、`docs/architecture.md` 20 章、各 `Cargo.toml`、`spikes/sample-load/Cargo.toml` | いまの meloet の形 | A（手もと） | 2026-11-21 |
| S30 | 同じ [S21] と [S22] を組んだ手順 | 順番 | C（手順は推論） | 2026-11-21 |
| S31 | https://raw.githubusercontent.com/juce-framework/JUCE/develop/modules/juce_audio_plugin_client/juce_audio_plugin_client_VST3.cpp（100000 文字から） | `setupProcessing` で `setNonRealtime(processMode == kOffline)`、`processAudio` で毎ブロック `data.processMode` を見る | A（要約を通して読んだ） | 2026-11-21 |
| S32 | https://www.bitwig.com/userguide/latest/exporting_audio/ | 「Real-time は標準の offline の書き出しを実時間にする」 | A | 2026-11-21 |
| S33 | https://www.github.com/pdeljanov/Symphonia/issues/274（検索の要約） | 0.5.4 の mp3 のシークのノイズ | B | 2026-11-21 |
| S34 | https://github.com/Remade-With-Rust/remade_ffmpeg_rs | 「FFmpeg の再実装。FFmpeg のコードは使っていない」、Apache-2.0、`.claude` あり | B（本人の言葉。AI の件は C） | 2026-11-21 |
| S35 | https://www.iis.fraunhofer.de/en/ff/amm/prod/audiocodec/audiocodecs/mp3.html（検索の要約） | mp3 のライセンス事業は 2017-04-23 に終了、最後の特許が切れた。ただし実装ごとの特許が残るかもしれない | A | 2026-11-21 |
| S36 | https://hydrogenaudio.org/ のスレッド（検索の要約）、LAME のタグの話 | LAME の遅れ 576 | C | 2026-11-21 |
| S37 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Workflow+Diagrams/Audio+Processor+Call+Sequence.html | 呼びの順（`setupProcessing` は Setup Done の前、`setActive`、`setProcessing`） | A | 2026-11-21 |
| S38 | 手もと `/usr/include/lv2/worker/worker.h` 131〜160 行 | free-wheeling のとき worker をその場で動かしてよい | A | 2026-11-21 |
| S39 | https://raw.githubusercontent.com/juce-framework/JUCE/develop/modules/juce_audio_plugin_client/juce_audio_plugin_client_LV2.cpp | `setNonRealtime(freeWheeling > 0.5)`、ポートの宣言 | A | 2026-11-21 |
| S40 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/session_export.cc ・ `plugin_insert.cc` | 書き出しの始まり（freewheel / realtime）、pre-roll、`flush_all_inserts` | A（要約を通して読んだ） | 2026-11-21 |
| S41 | https://forum.juce.com/t/trouble-with-isnonrealtime-for-offline-render/18564 | REAPER に offline を知らせる設定がある（名前は無い） | C（個人の投稿） | 2026-11-21 |
| S42 | https://forum.juce.com/t/ableton-vst3-isnonrealtime-not-set/46912 | Ableton が VST3 で `kRealtime` のまま | B（JUCE の開発者本人の報告。公式ではない） | 2026-11-21 |
| S43 | Steinberg のフォーラム、Cantabile の文書（検索の要約） | 外部エフェクトで実時間を強制、offline は機器に結ばれたプラグインと合わない | C / B | 2026-11-21 |

食いちがい: 特にない。公式の表（S4）は master、手もとのコードは 0.6.1 で、表の「mp3 は gapless あり」は 0.6.1 の demuxer のコード（S9）と合う。LV2 の `isLive` を使うホストの作法は、一次情報では見つからなかった（仕様の文だけ）。
