# 歌と音のファイルを、曲のテンポと高さに合わせる部品（タイムストレッチ、ピッチシフト、テンポ推定）

調べた日 2026-11-21（meloet-research）。確かさは A・B・C。[S番号] は最後の表。GitHub などの中身は、取りこみの道具が要約して返したものを読んだ。重要な所は別の面からも見て確かめた（Signalsmith の `signalsmith-stretch.h`、`fft.h`、Bungee.h は 2 回以上）。

## 結論（先に）

1. **推す 1 つ: Signalsmith Stretch**（MIT。C++ のヘッダだけの部品）。理由は 4 つ。
   - ライセンスが MIT。LICENSE.txt に 「Copyright (c) 2022 Geraint Luff / Signalsmith Audio Ltd.」とある（A [S7]）。
   - 声のための道具がそろっている。`setFormantFactor` で声の感じ（フォルマント）を保てる。半音は小数でも入る。
   - **ブロックごとに、入る長さと出る長さを変えるだけで比率を変えられる**。だんだんのテンポに合う（A [S5]）。
   - 作者が活発。最後のコミットは 2026-10-22（A [S9]）。
   - **ただし 2 つ、はっきりしない所がある**。
     - 声での良さを、第三者が比べた聞きくらべが見つからなかった。作者の README は、フォルマントの補正は「PSOLA のような単音向けの方法ほど鋭くない」と自分で書く（A [S5]）。
     - 1 本あたりの CPU の数字が、どこにも見つからなかった。
     - どちらも、棟梁が自分の歌の素材で聞き、測る必要がある（5 章の試験）。
2. **次の 1 つ: Bungee**（MPL-2.0。C++）。
   - ライセンスは通る。MPL-2.0 の部品は、ファイルを書きかえずに使う、という決まりに合う。
   - 時間と高さを 1 粒（グレイン）ごとに変えられ、Linux・Windows・Mac の出来合いの版がある（A [S17] [S18]）。
   - **ただし、フォルマントを保つ設定がない**（ヘッダに「formant の設定はない」。A [S18]）。
   - だから、歌の高さ変えで「声の感じを保つ」を既定にする、という仕様は、Bungee だけでは満たせない。
   - 使うなら「テンポに合わせる（高さは変えない）」の係としてだけ。
3. **ライセンスの都合で使えない物**
   - **Rubber Band**: GPL（v2 以降）か商用。商用ライセンスは買わない決まりなので、出荷には使えない（A [S1]）。
   - **SoundTouch**: LGPL v2.1（商用は依頼しだい）。「LGPL は使わない方向」に当たる（A [S22]）。
   - **aubio**（GPL-3.0 以降）、**Essentia**（AGPL-3.0）: テンポを推す物。どちらも入れられない（A [S30] [S31]）。
   - **élastique / ZTX**: 商用の SDK。条件のページは読めなかった（B [S36]）。
4. **仕様どおりにできない所の案**
   - 仕様どおり（フォルマントを保つ、±12 半音、±50 セント、だんだんのテンポ）は、Signalsmith で技術的にはできる。
   - もし聞いて「声が崩れる」となったときの逃げ道は 3 つ。
     - (a) 声の高さ変えの範囲をせまくして、フォルマント補正をオンのままにする。
     - (b) Rubber Band の商用ライセンス（決まりで買わない。オーナーさんに聞く話になる）。
     - (c) 自分で位相ボコーダと声の包絡（スペクトルの形）の補正を書く。文献は 4 章。大きな仕事。
   - 無料で、声に強く、MIT か Apache か MPL の部品は、他に見つからなかった。
5. **鳴らしかた（C、推論）**: 先に「作って、しまっておく」ほうを、はじめの道にするのがよい。
   - 理由は、歌のファイルは先読みでき、音のスレッドにメモリを取らせずにすむから。
   - 鳴らしはじめの重さのかたまり（HISE の例、C [S35]）も、音のスレッドの外で先に吸収できる。
   - その場で鳴らすのは、測ってからの 2 番手。
6. **テンポをさがす**
   - 歌（アカペラ）には、ほぼ意味がない。拍がないので。
   - ループ素材では、「小節数が整数になる長さ」から候補を絞る道が素直（C、推論）。
   - 部品を使うなら、`stratum-dsp`（MIT か Apache-2.0。Rust）が条件に合う。ただし作者の自己申告で、DJ 向けの曲 155 本、±2 BPM 以内が 87.7%。別のデータ（FMA Small）では 56.7%（A [S29]）。
   - 声やループの正しさは測られていない。聞いて直す前提の「推す」にとどめる。

## くらべる表

「A」などの印は、その欄を直接支える出どころのいちばん高い確かさ。

| 観点 | Signalsmith Stretch | Bungee | Rubber Band | SoundTouch | timestretch（Rust crate） |
|---|---|---|---|---|---|
| 1. ライセンス | **MIT**（LICENSE.txt。A [S7]）。入れている DSP の `dsp/` も MIT（A [S5]）。Rust の包み `signalsmith-stretch` 0.1.3 も MIT（A [S14]）。FFT の `signalsmith-linear` も MIT（A [S10]） | **MPL-2.0**（ヘッダと README。A [S17] [S18]）。中で使う Eigen は MPL-2.0 と表示（A [S37]）、PFFFT のライセンスは**確かめていない** | **GPL v2 以降か商用**（A [S1]） | **LGPL v2.1**（商用は依頼しだい。A [S22]） | MIT（A [S24]）。リポジトリの LICENSE は見ていない |
| 2. 声での良さ | **第三者の聞きくらべは見つからない**。作者の README は、フォルマントの補正を「単音向けの方法ほど鋭くない」と書く（A [S5]）。作者の言葉として、声には既定より短いブロックを勧める（C、検索の要約を通しただけ [S38]） | 第三者の声の評価は見つからない。公式サイトの「声にも使える」は商用の Pro 版の宣伝（B [S21]）。聞きくらべの表はある（`bungee.parabolaresearch.com/compare`）が、勝ち負けは書いていない（B [S20]） | R3 エンジンは「声、やわらかい立ち上がり」で R2 より良いと公式 README が書く（A [S1]）。ただし使えない | 時間領域の方法。声の音は未確認。WSOLA は早送りで崩れる、という声がある（C [S39]） | EDM 向け（A [S24]）。声への言及なし |
| 3. フォルマント | **ある**。`setFormantFactor(倍率, compensatePitch)`、`setFormantBase(基音の目安)`（0 なら自動で推す）。コードを読むと、**`transpose` を入れて `setFormantFactor(1.0, true)` にすると、声の感じが元の位置に残る**。`false` だと、補正の段が飛ばされ、声の感じも高さといっしょに動く（A [S6]、読みはコードからの推論なので C 相当の読みも含む。聞いて確かめる） | **ない**（ヘッダに「formant の設定はない」。A [S18]） | あり。`OptionFormantPreserved`（声や声入りの混ぜに勧める）。R3 は高さと独立にずらせる（A [S3] [S4]） | 見つからない。時間のばしと、サンプル率変換の合わせ技なので、保たれないと推せる（C [S23]） | `pitch_shift` の経路だけに、包絡の補正がある（Off、Balanced、Vocal）。ストレッチの経路にはない。最新の docs は「フォルマント保存はこのプロセッサの範囲外」（A [S24] [S25]） |
| 4. その場で鳴らす速さ・遅れ | CPU の数字は**見つからない**（わからない）。遅れは `inputLatency()` と `outputLatency()` の 2 つ。既定の 48 kHz で、窓 5760 こま（0.12 秒）、間隔 1440 こま（0.03 秒）。`presetCheaper` は 4800 と 1920。`process()` に `new` はなく、`configure()` で全部の大きさを決める（A [S6]）。**はじめに窓 1 つぶんの音を食べさせる必要がある**（HISE の例は約 50 ms。C [S35]）。`splitComputation` で、計算を均して遅れを 1 間隔ふやす（A [S5]） | 約 100 粒/秒（A [S17]）。遅れの数字も CPU の数字もなし。メモリの扱いも書いていない（A [S18]） | R3 の開始の遅れは、短い窓で 1280 こま、既定で 2048 こま（A [S4]）。`RubberBandLiveShifter` は高さだけ | 見つからない | 「処理にメモリ取りも鍵もない」と書く。12.7 ms のキーロックの遅れ（0.9.1。A [S24] [S25]） |
| 5. 時間とともに比率を変える | **できる**。`process()` に、長さのちがう入力と出力を渡す。大きさの上限はない。整数なので「ならして目的の比率になる」ように呼ぶ側が管理する。「0.75〜1.5 倍がいちばんよい」（A [S5]） | **できる**。粒ごとに速さと高さを指す。ゼロや逆もできる（A [S17] [S18]） | できる（R3 のリアルタイム。A [S4]）。使えない | 見つからない | 予約で変える（`StreamSession::schedule`）。ブロックごとの細かさは書いていない（A [S25]） |
| 6. 使いかた・保守 | Rust の包みあり（0.1.3、2025-09-18、C++ を `cc` で作る。bindgen が要る。A [S13] [S14]）。**包みが取りこんだ Signalsmith は古いコミット `c598726`、本体は 2026-10-22 まで動いている**（A [S15] [S9]）。包みのコミットは 15 個。自分で薄い包みを書く道もある。C++11 のヘッダだけ。Clang での試験が主。Apple Clang 16.0.0 と `-ffast-math` の組み合わせは `#error` で止まる（A [S5] [S11]） | C++。CMake。Eigen と PFFFT が要る。出来合いの版が Linux・Windows・macOS・Android・iOS（A [S17]）。Rust の包みは**見つからなかった**。Ableton の複製がある（A [S19]） | 使えない | 2.4.1、2026-03-29（A [S22]） | 純 Rust（`#![forbid(unsafe_code)]`）。0.9.1（2026-08-02）から 0.16.0（2026-10-29）まで約 3 か月で 7 版。**作りが動いている最中**（A [S24] [S25]） |

## 1. ライセンスの見かた

- **Rubber Band**（A [S1]）
  - README は、GPL v2 以降だと書く。それ以外の条件で配るには、作者から商用ライセンスを買う。
  - 公式サイトでも「GPL。商用アプリへ入れるなら商用ライセンスを買う」（B [S2]。ページは今日読んだが、4.0 の日が 2024-10-25 で 2 年をこえるので、1 段下げた）。
  - `RubberBandLiveShifter`（高さだけ）が 4.0 の新しい API（B [S3]）。
- **Signalsmith Stretch**
  - リポジトリの LICENSE.txt は MIT（A [S7]）。
  - 作者は README で「ほかの条件がいるなら連絡を」とも書く。
  - ヘッダ（`signalsmith-stretch.h`）には、ライセンスの文はなかった（A [S6]）。LICENSE.txt に頼ることになる。
  - 下の `signalsmith-linear`（FFT など）も MIT と表示（A [S10]）。
  - FFT の既定は、組みこみの `SimpleFFT`（2 の累乗の FFT）で、外の物は要らない（A [S11]）。PFFFT、Accelerate、IPP は、マクロで選んだときだけ（A [S10] [S11]）。
- **Bungee**
  - ヘッダの MPL-2.0、著作者は Parabola Research Limited（2020-2026。A [S18]）。
  - 基本版（Basic）は公開、Pro 版は商用（A [S17] [S18]）。
  - Eigen は MPL-2.0 と表示（A [S37]）。ただし GitLab の画面で、MPL のファイル名と COPYING.APACHE のリンクが食いちがって見えた。ファイルそのものを入れる前に、LICENSE を見る必要がある。
  - 版は、元のリポジトリの README が「main の全コミットに版札がつく」と書く（A [S17]）。Ableton の複製は 2.4.15 ベース（A [S19]）。
- **SoundTouch**: LGPL v2.1。商用（GPL の縛りがない）は依頼しだい（A [S22]）。
- **élastique（zplane）**: 商用の SDK として載っている。条件は、そのページからは読めなかった（B [S36]）。
- **aubio**: GPL-3.0 以降（A [S30]）。**Essentia**: AGPL-3.0（A [S31]）。
- **Rust の `wsola` 0.1.0（2026-07-13）**: MIT。高さを変える機能は書いていない（A [S26]）。
- **`cute-dsp` 0.0.41（2026-06-17）**: MIT。「Signalsmith DSP の Rust 版」と README が書き、`stretch` の部品がある（A [S28]）。
  - 作った人は別の人。点検の記録、元との音のちがいは見つからなかった（C）。
  - 純 Rust で書けるなら C++ を連れてこなくて済むが、**中身を読んでいない**。有望だが調べていない物として残す。

## 2. 声での良さ

- 見つかった**第三者の聞きくらべ**は、Bungee の作者のページ「Comparison of audio stretch techniques」（日付 2026-01）だけ（B [S20]）。
  - Audacity、Bungee 2.4.8、Bungee Pro、élastique、PaulStretch、Rubber Band 4.0.0、Serato Studio、**Signalsmith Stretch**、Sonic、SoundTouch 2.3.1、Superpowered を、短い音楽と声の素材（歌の素材もある）でならべている。
  - **ページには勝ち負けも順位も書いていない**。耳で聞いてください、という作り。
  - 持ち主が Bungee の作者なので、公平さの断りも付いている。「データは古くなったり、まちがいがありうる」と書いてある。
  - 数の表は、周波数の誤差、音の立ち上がりのずれなど。声の良さを数にはしていない。
- Signalsmith の作者は、声のストレッチの話で「Rubber Band のほうがよい例を教えてほしい」と、掲示板で言った、という検索の要約がある（C [S38]。掲示板本体は読めなかった）。
- 設計の記事（作者）には、**弱点**がはっきり書いてある（B、作者の言葉 [S8]）。
  - 大きなのばしで、立ち上がりが別の所にくり返される（時間のエイリアス）。いまの回避は、縦の位相のスケールを 2 倍までに限り、長いのばしでは乱数を足す。
  - 立ち上がりを守る非線形の時間の地図は「これから」で、未実装。
  - フォルマントと声については書いていない。
- 位相ボコーダの弱点は文献にもある（A [S33] [S34]）。
  - Laroche と Dolson（1999）の位相ロックは、「にじみ（phasiness）」を減らす。ただし立ち上がりでは縦の位相のつながりを守れない。Röbel が立ち上がりの直しを提案した。
  - Driedger と Müller（2016）の総説は、OLA、WSOLA、位相ボコーダ、そして打楽器と調和の分離を足す方法を、できることとできないことの形で整理している。この総説の中身（結論や数）は、題と目次しか読んでいない。

## 3. フォルマントを保つ作法

- **位相ボコーダでの作法の文献**
  - Röbel と Rodet「Efficient spectral envelope estimation and its application to pitch shifting and envelope preservation」DAFx-05（A [S32]）。ケプストラムを繰り返して直す「真の包絡（true envelope）」で、声のスペクトルの形を測る。高さを変えるとき、その形を元の所に保つ。
  - 同じ著者の「Real Time Signal Transposition with Envelope Preservation in the Phase Vocoder」は、計算を 2〜9 分の 1 にして、リアルタイムを現実的にした（B [S32]。検索の要約）。
- **Signalsmith のやりかた**（A [S6]、コードの要約を通した）
  - 入力の包絡（ピークとくぼみから作る尺度。2025-08 のコミット「Formant metric based on peaks/dips」）を測る。
  - 出力の周波数の位置で、エネルギーの比（変換後の位置の包絡 ÷ 元の位置の包絡）を掛ける。
  - `setFormantBase()` に基音の目安を入れる（0 なら自動で推す）。人の声は 100〜300 Hz あたりなので、README の例は `setFormantBase(200/sampleRate)`（A [S5]）。
  - `setFormantFactor(1.0, true)` で声の感じが元の位置に残る、は**コードを読んだ推論**（C 相当）。補正は「なめらかな尺度」なので近似。**聞いて確かめる**。
  - `tonality limit`（高い所は 1:1 で素通し）も、音色を少し守る道として README にある（A [S5]）。
- **Rubber Band**: `OptionFormantPreserved` を、高さを変える声に勧める。リアルタイムの声向けの設定の例も公式にある（A [S4]）。使えないが、「聞いて比べる基準」としては出荷しない形で使えるかもしれない（C、推論。オーナーさんに聞く）。

## 4. 自分で書く場合の文献

- 位相ボコーダのにじみを減らす: Laroche と Dolson、IEEE Trans. Speech and Audio Processing 7(3)、1999、323–332、DOI 10.1109/89.759041（A [S33]）。
- 立ち上がりの扱い: Röbel（2003、DAFx-03、題は「A new approach to transient processing in the phase vocoder」）。この題と年は、Laroche と Dolson の検索結果の中の「Röbel が直しを出した」という記述から出した推測で、論文そのものは読んでいない（C）。
- 声の包絡を保つ: Röbel と Rodet（2005、DAFx-05）（A [S32]）。
- 全体のよくある方法の整理: Driedger と Müller（2016、Applied Sciences 6(2):57、題と目次を確認）（A [S34]）。
- 高さは「時間をのばして、サンプル率を変えて元の長さにする」で作る方法が昔からあるが、フォルマントも動く。Signalsmith は「リサンプルしない、時間と高さを別々に扱う」と書く（B [S8]）。

## 5. その場で鳴らす／しまっておく：作りかたと試験（C、推論）

入力が「ファイル」で、先読みできる、という事実から出した案（A の事実からの C の推論）。

- **しまっておく道**（先に作る）
  - 別のスレッドで、ファイルを読んで Signalsmith に通し、できた波を持つ。音のスレッドは、持っている波を読むだけ。
  - メモリは先に取れる。鍵も要らない。
  - テンポ、高さ、フォルマントが変わったら作りなおす。作っている間は、もとの音で鳴らす。
  - 作りなおしの間の「曲のテンポの地図」を、入力の長さの計算（下）に使う。
- **ブロックごとの比率**
  - 曲のテンポの地図から、出力のこま `t0〜t1` に対応する入力のこま位置 `p(t0)〜p(t1)` を出す。
  - そのブロックで `process()` に、入力 `p(t1)−p(t0)` こま、出力 `t1−t0` こまを渡す。
  - 小数の端数は、位置の合計（累積）で持つ。だんだんのテンポは、テンポの地図の積分でそのまま出る。
  - README が言う「ならして目的の比率になるように」は、この累積でできる。
- **その場で鳴らす道**（あとで測ってから）
  - `splitComputation` を立てて、`seek()` で先に温めるのを、音のスレッドの外でやる。
  - 音のスレッドで `process()` が `new` しないことは、ソースの読みでは「ほぼ」（A [S6]）。
    - `configure()` が `tmpProcessBuffer` などの大きさを決める。
    - `peaks.emplace_back` は、`bands/2` を先に確保しているので、ふつうは足りる（ピークはバンドの半分以下になるはず、という推論は C）。
    - STFT の中は、`Input` と `Output` のコピー代入を除いて、`configure()` の後は取りに行かない（A [S12]、要約を通した）。
  - ただし、**作者は「音のスレッドで安全」とは言っていない**（A [S5]）。meloet の機械の見はり（`crates/meloet-sound` と同じ型のもの）で試す。
  - 開始の重さは、HISE の例では、音の出はじめの 1 ブロックに集まって CPU が跳ねた。作った人が「ストリーム用スレッドで先に計算する」形に直した（C [S35]）。
- **聞いて測る試験**（この調べの外）
  - 同じ歌（話し声ではなく歌）を、Signalsmith、Bungee、（基準として）Rubber Band の R3 に通す。
  - 条件は、ストレッチ 0.75〜1.5 倍、半音 ±3 と ±12、フォルマント補正の入り切り。
  - オーナーさんの耳か、聞きくらべのページ（[S20]）の作りをまねて聞く。
  - CPU は、1 本あたり（48 kHz のモノラルとステレオ）で、音のスレッドの 1 ブロックの最悪時間を測る。
  - 声には、既定の窓（0.12 秒）より短い窓を、作者が勧めている（C [S38]）。短い窓は、`new(channels, block_length, interval)` で決める（A [S13]）。

## 6. 使いかた（Rust から）

- `signalsmith-stretch` 0.1.3（A [S13] [S14]）
  - `Stretch::new(channels, block, interval)`、`preset_default`、`preset_cheaper`。
  - `set_transpose_factor_semitones(半音, tonality_limit)`、`set_formant_factor(倍率, compensate_pitch)`、`set_formant_factor_semitones`。
  - `process(&mut self, input, output)`（長さのちがいがストレッチ）、`seek`、`flush`、`reset`、`input_latency`、`output_latency`、`exact`。
  - 声の基音を指す関数は `signalsmith_stretch_set_formant_base(&self, …)` という名前と形（名前がクレート名つきで、`&self`）。
  - `unsafe impl Send/Sync` がある。理由は書いていない。
  - ビルドは `cc` で C++14、`bindgen` が要る（最適化のフラグは指定していない）（A [S16]）。ビルドの `opt-level` は `Cargo.toml` で決める必要がある。作者は「最適化なしだと約 10 倍遅い。デバッグでも最適化を」と言う（A [S5]）。meloet は、すでに `meloet-sound` を opt-level 3 にしている。
  - docs.rs の画面に出る対象は x86_64-unknown-linux-gnu だけだが、これは docs.rs のビルドの対象で、対応の宣言ではない（C）。Windows と Mac は書いていない。
  - 取りこんでいる Signalsmith が古いコミット（`c598726`）。新しい `outputSeek`（2025-08 追加）などを使いたいなら、自分で薄い包みを書くほうが素直（C、推論）。
- **Bungee**: C++ の `Stretcher<Edition>` のテンプレートと、C の関数表（`getFunctionsBungeeBasic()`）。後者は C のリンクなので、Rust からつなぎやすい（A [S18]）。CMake、Eigen、PFFFT、サブモジュール。
- **Mac/Windows（第 9 回）**
  - Signalsmith: Apple Clang 16.0.0 と `-ffast-math` で止まる（A [S11]）。MSVC は、読んだ範囲では情報なし（わからない）。
  - Bungee: 出来合いの版は 5 つの環境にある（A [S17]）。

## 7. テンポをさがす

| 候補 | ライセンス | 中身 | 確かさ・注意 |
|---|---|---|---|
| aubio | GPL-3.0 以降 | 入れられない | A [S30] |
| Essentia | AGPL-3.0 | 入れられない | A [S31] |
| QM Vamp plugins | **調べていない**（Vamp は GPL の流れが多いと記憶しているが、確認なし。C） | | わからない |
| `stratum-dsp` 1.0.0（2025-12-19） | MIT か Apache-2.0 | 純 Rust。FFT とオートコリレーションの 2 つのテンポグラム（Grosche ほか 2012）、HMM のビート追い。全曲の入ったメモリ（`Vec<f32>`）を渡す | 自己申告: 155 本で ±2 BPM が 87.7%、別の FMA Small では 56.7%、オクターブのまちがいは約 12%。3 分の曲で約 0.2 秒。作者は 1 人で、版は 1 つだけ。声は言及なし（A [S29]） |
| `timestretch` の `analysis` 機能 | MIT | 50〜220 BPM のテンポグラム（オートコリレーション）とビート追い。正しさの数なし | A [S24] [S25]（機能の存在）。正しさはわからない |
| `beat-this` 0.3.0 | MIT（クレート）。**学習済みのモデルのライセンスは確かめていない** | ニューラル。モデルが要る | B（検索の要約 [S29 の検索結果]）。確かめが要る |
| 自分で書く | — | onset の強さ（スペクトルの流れ）→ オートコリレーションまたはテンポグラム → 候補（BPM の倍と半分も） | 文献は [S29] の Grosche ほか 2012 |

- 歌のファイルは、拍の強い音がほとんどない。テンポ推定は、元の曲のテンポ（ユーザーが入れる、または録ったときの曲のテンポ）を使うほうが確か（C、推論）。
- ループには、次の安い道がある（C、推論。すべて未検証）。
  - ファイルの長さと「1 小節の数」の候補（1、2、4、8 小節）から、60〜180 BPM に入る BPM を並べる。
  - 次にオンセットのオートコリレーションで選ぶ。
  - ファイルの名前や、WAV などのメタデータに入っている BPM も読む。形式の読みかたは**調べていない**。
- 「テンポを推す」は、あくまで「推す」の表示で、ユーザーが直せる形にするのがよい（C、設計の提案）。

## わからなかったこと・追加で調べるとよいこと

- Signalsmith Stretch の **CPU の数字**（1 本あたり）、**声での良さの第三者評価**。どちらも見つからなかった。5 章の試験で自分で測る。
- Signalsmith の **MSVC（Windows）での確かめ**。README は「主に Clang で試験」と書く（A [S5]）。第 9 回の前に、小さな試験を。
- Bungee の **PFFFT のライセンス**、**Eigen の LICENSE ファイル**（GitLab の画面で、MPL と COPYING.APACHE の表示が食いちがって見えた）。
- Bungee の **メモリの取りかた**、**遅れの数字**（どちらも文書になし）。
- **`cute-dsp`**（Rust 版の Signalsmith）の中身と、元との音のちがい。
- **QM Vamp plugins のライセンス**。
- 作者の掲示板の話（声には短い窓）は、検索の要約だけで、本文は読めなかった。
- 一次の論文は、Röbel と Rodet（2005）の要約と Laroche と Dolson（1999）の要約までで、本文の数式は読んでいない。
- Rubber Band の「最後の版の日付 2024-10-25」は 2 年をこえている。いまも GPL か商用か、は公式ページと README で確認したが（今日）、将来変わるかもしれない。
- 取りこんだ GitHub などの中身は、要約を通している。ライセンスの文（LICENSE.txt）は、型と著作者の行だけを確かめた。出荷の前に、配る物の中の LICENSE ファイルを自分の目で読むこと。

## 出どころ

| ID | URL | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | https://github.com/breakfastquay/rubberband と https://raw.githubusercontent.com/breakfastquay/rubberband/default/README.md | GPL v2 以降、または商用。.NET の包みだけ BSD 系。依存（FFTW3 は GPL か商用、IPP は商用、他は BSD 系）。R3 は声に向く。RubberBandLiveShifter | A（版の日付が 2 年超のため B 相当の扱いも併記） | 2026-11-21 |
| S2 | https://breakfastquay.com/rubberband/ | 4.0、2024-10-25。GPL、商用は購入 | B（2 年超で 1 段下げ） | 2026-11-21 |
| S3 | https://raw.githubusercontent.com/breakfastquay/rubberband/default/CHANGELOG | 4.0 の変更（LiveShifter）。フォルマント保存は 1.2、高さと独立のフォルマントは 3.0（R3） | A（2 年超で B 相当） | 2026-11-21 |
| S4 | https://breakfastquay.com/rubberband/integration.html | R3 の開始の遅れ（1280、2048 こま）、`OptionFormantPreserved`、リアルタイムの声の設定例 | A | 2026-11-21 |
| S5 | https://raw.githubusercontent.com/Signalsmith-Audio/signalsmith-stretch/main/README.md | MIT、`setFormantFactor`、`setFormantBase`、tonality limit、遅れ、`splitComputation`、比率は入出力の長さで、0.75〜1.5 倍がよい、Apple Clang の注意 | A | 2026-11-21 |
| S6 | https://raw.githubusercontent.com/Signalsmith-Audio/signalsmith-stretch/main/signalsmith-stretch.h | 確保の場所、formant のコード、preset の数、遅れの式、版 1.3.2、`processFormants` の条件 | A（要約を通した） | 2026-11-21 |
| S7 | https://github.com/Signalsmith-Audio/signalsmith-stretch/blob/main/LICENSE.txt | MIT、Copyright (c) 2022 Geraint Luff / Signalsmith Audio Ltd. | A | 2026-11-21 |
| S8 | https://signalsmith-audio.co.uk/writing/2023/stretch-design/ | 設計、弱点（時間のエイリアスなど）。フォルマントと声は書いていない | B（作者の言葉） | 2026-11-21 |
| S9 | https://github.com/Signalsmith-Audio/signalsmith-stretch/commits/main | 最後のコミット 2026-10-22。2025-08 の formant metric のコミット | A | 2026-11-21 |
| S10 | https://github.com/Signalsmith-Audio/linear | MIT、FFT の外部の道（Accelerate は Mac の既定、IPP は既定オフ、PFFFT はマクロで選ぶ） | A | 2026-11-21 |
| S11 | https://raw.githubusercontent.com/Signalsmith-Audio/linear/main/fft.h | 既定は組みこみの SimpleFFT、`fft()` は確保しない、`resize()` で確保。Apple Clang 16.0.0 の `#error` | A（要約を通した） | 2026-11-21 |
| S12 | https://raw.githubusercontent.com/Signalsmith-Audio/linear/main/stft.h | `configure()` で全部の大きさを決める。処理中は確保しない（コピー代入を除く）。遅れの式 | A（要約を通した） | 2026-11-21 |
| S13 | https://docs.rs/crate/signalsmith-stretch/0.1.3/source/src/lib.rs と https://docs.rs/crate/signalsmith-stretch/0.1.3 | 公開関数、0.1.3 は 2025-09-18、0.1.0 は 2024-12-02、依存 | A | 2026-11-21 |
| S14 | https://docs.rs/crate/signalsmith-stretch/0.1.3/source/Cargo.toml | MIT、リポジトリ、ビルド依存（bindgen、cc） | A | 2026-11-21 |
| S15 | https://github.com/colinmarc/signalsmith-stretch-rs | MIT、コミット 15、Signalsmith はコミット `c598726` | A | 2026-11-21 |
| S16 | https://raw.githubusercontent.com/colinmarc/signalsmith-stretch-rs/main/build.rs | C++14、フラグなし | A | 2026-11-21 |
| S17 | https://github.com/bungee-audio-stretch/bungee と README | MPL-2.0、粒ごとの速さと高さ、Eigen と PFFFT、出来合いの版の環境、Pro は商用 | A | 2026-11-21 |
| S18 | https://raw.githubusercontent.com/bungee-audio-stretch/bungee/main/bungee/Bungee.h | Request、formant の設定なし、確保とリアルタイムは書かず、MPL-2.0 ヘッダ | A（要約を通した） | 2026-11-21 |
| S19 | https://github.com/Ableton/bungee | Ableton の複製、MPL-2.0、2.4.15 ベース | A | 2026-11-21 |
| S20 | https://bungee.parabolaresearch.com/compare-audio-stretch-tempo-pitch-change | 聞きくらべの表。順位なし。注意書きあり。日付 2026-01 | B（Bungee の作者） | 2026-11-21 |
| S21 | https://bungee.parabolaresearch.com/ | 基本版と Pro 版。声の宣伝は Pro | B | 2026-11-21 |
| S22 | https://codeberg.org/soundtouch/soundtouch | LGPL v2.1、商用は依頼しだい、2.4.1（2026-03-29） | A | 2026-11-21 |
| S23 | 検索の要約（SoundTouch と formant） | WSOLA 系、フォルマントの保存はなさそう | C | 2026-11-21 |
| S24 | https://docs.rs/crate/timestretch/0.9.1 | MIT、EDM 向け、確保なし、フォルマントは `pitch_shift` のみ、依存、2026-08-02 | A | 2026-11-21 |
| S25 | https://docs.rs/crate/timestretch/latest | 0.16.0（2026-10-29）、予約で変える、フォルマントは範囲外、限りの一覧 | A | 2026-11-21 |
| S26 | https://docs.rs/crate/wsola/0.1.0 | MIT、純 Rust、高さの変更なし、2026-07-13 | A | 2026-11-21 |
| S27 | 検索の要約（`pitch_shift` 2.1.0、`rocoder`、`phase_vocoder`） | `pitch_shift` は MIT、遅れ `1024 − out_samples`。ほかは未熟 | C | 2026-11-21 |
| S28 | https://docs.rs/crate/cute-dsp/latest | MIT、0.0.41（2026-06-17）、Signalsmith の Rust 版と称する | A（README の自己申告。元との一致は未確認） | 2026-11-21 |
| S29 | https://docs.rs/crate/stratum-dsp/latest | MIT か Apache-2.0、1.0.0（2025-12-19）、自己申告の正しさ | A（正しさの数は作者の自己申告） | 2026-11-21 |
| S30 | https://github.com/aubio/aubio | GPL-3.0 以降 | A | 2026-11-21 |
| S31 | https://github.com/MTG/essentia | AGPL-3.0 | A | 2026-11-21 |
| S32 | https://dafx.de/paper-archive/details/EDSTSaO17Gd_0LV03iwDfw ほか（検索の要約） | Röbel と Rodet（2005）。true envelope、高さ変えでの保存。続報はリアルタイム化 | A（論文の所在）/B（続報の中身） | 2026-11-21 |
| S33 | https://www.ee.columbia.edu/~dpwe/papers/LaroD99-pvoc.pdf（検索の要約） | Laroche と Dolson（1999）の位相ロック | A（書誌）、本文は未読 | 2026-11-21 |
| S34 | https://mdpi.com/2076-3417/6/2/57（検索の要約） | Driedger と Müller（2016）の総説 | A（書誌）、本文は未読 | 2026-11-21 |
| S35 | https://forum.hise.audio/post/87979 | HISE の人の言葉。CPU はよいが、出はじめに跳ねる。約 50 ms 要る。直した | C | 2026-11-21 |
| S36 | https://licensing.zplane.de/ | élastique は SDK として載っているが、条件は読めなかった | B | 2026-11-21 |
| S37 | https://gitlab.com/libeigen/eigen | Eigen は MPL-2.0 と表示（ファイル名の表示が食いちがう） | A（食いちがいあり） | 2026-11-21 |
| S38 | 検索の要約（Reddit r/DSP の話、作者の言葉。掲示板本体は読めなかった） | 声には短い窓を勧める | C | 2026-11-21 |
| S39 | 検索の要約（Audacity の議論） | WSOLA は 2 倍をこえると崩れる、という声 | C | 2026-11-21 |

2 年より古い情報: Rubber Band 4.0（2024-10-25）の版と日付は、1 段下げた（B）。論文（1999、2005、2016）は変わりにくいので A のまま。ほかの版とライセンスは、2026 年に確認した物。

食いちがい: Bungee の公式サイト（[S21]）は、ライセンスの名前を書かない一方、GitHub のヘッダと README（[S17] [S18]）は MPL-2.0。高いほう（A）を採った。Eigen は GitLab の画面でライセンスの表示とファイル名が食いちがう（[S37]）。
