# テンポの「だんだん」と、オートメーションの書きかた

調べた日 2026-11-20（meloet-research）。確かさは A（公式のマニュアル・仕様・公式リポジトリのソース）・B・C。[S番号] は最後の表。棟梁が `docs/research/tempo-and-automation.md` に保存してください。

## 結論

### 1. テンポの「だんだん」

- **拍にまっすぐ、と確かめられた DAW は Ardour だけ（A）。** Ardour はテンポを拍に対してまっすぐ動かす。式は対数と指数。ソース [S1]、ヘッダ [S2]、作者の導きの書類 [S3] が食いちがいなく一致した。
- **仕様書の例（104 → 120、7 小節目が 112.00）は拍にまっすぐで合う（A、依頼文の例を計算）。** 時間にまっすぐだと 7 小節目は約 112.29 になる。
- **ほかの DAW は、公式の文書でわからなかった。** REAPER、Cubase、Logic、Bitwig、Ableton、Studio One、FL Studio、MuseScore のマニュアルは、どちらにまっすぐかを書いていない。一次情報は見つからなかった。
  - Logic、Pro Tools、MuseScore は、だんだんを細かいテンポの段で作る（A、B）。
  - Cubase の「Ramp」は SMF（標準 MIDI ファイル）に出ない、という報告がある（C）。
  - 本当に知りたいなら、実機で測るしかない（書き出した SMF の段を数える）。
- **式（C が入らない、導き付き）:**
  - 拍にまっすぐ: `t(b) = (1/k)·ln(1 + k·b/T0)`、`b(t) = (T0/k)·(e^{k·t} − 1)`。
  - 時間にまっすぐ: `b(t) = T0·t + a·t²/2`、`t(b) = 2b / (√(T0² + 2ab) + T0)`。
  - 0 で割る形になるのは、k や a が 0 に近いとき。`log1p` と `expm1`、近いときは級数で逃げる（4 章）。
- **SMF へ書くときは、だんだんを段にする。** SMF のテンポは 1 つの値（4 分音符あたりのマイクロ秒、24 ビット）で、「だんだん」を表す事がらはない（B）。ほかの DAW も段にする。Logic は「Density」で段の細かさを選ばせ、各段をその間の平均テンポにする（A、C）。

### 2. オートメーション

- **書くときの形はどの DAW もほぼ同じ（A）。**
  - Touch は、さわっている間だけ書き、離すと元の線に戻る。
  - Latch は、離しても最後の値で書きつづけ、止めると終わる。
  - Write は、通った所を全部書きかえる。Ardour は止めると Touch に戻し、曲に保存するときも Touch にする。
- **離したあと元の線に戻る速さは、設定で決められる。** Logic は「Ramp Time」、Cubase は「Return Time」、REAPER は「return speed」。
- **手で動かすと休む形があるのは Ableton Live と Bitwig（A）。**
  - 休んでいる間は、手で決めた値のまま。
  - 戻すのは、ボタンを押す（全部いっぺんに）。Live だけ、1 つのパラメータごとに右クリックでも戻せる。
  - 休みが曲に保存されるか、取り消しの対象か、どちらも文書にない（一次情報は見つからなかった）。
- **点の間引きは、Ardour だけ数がわかった（A）。** 30 ms ごとに値を取り、三角形の面積が 20 より小さい点を捨てる。Cubase は Reduction Level（%）、Bitwig は止めたあとに最適化、REAPER は「Reduce number of points」。
- **点と点の間の値の出しかたで、確かめられたのは Ardour だけ（A）。** 音量はフェーダーの目盛の位置でまっすぐ、Trim は dB でまっすぐ、パンはゲインでなく値そのままでまっすぐ。
- **書いた値の位置を、聞こえた位置に合わせる DAW は見つからなかった（A）。** Ardour は「いまの再生位置」にそのまま置く（オフセットなし）。Logic は人が決めるオフセット（ティック）を用意し、理由に「音の機器の遅れ」を挙げている。

## 1. テンポの「だんだん」のくわしい中身

### 1-1. DAW ごとの形

| DAW | 拍か時間か | 出どころ | 確かさ |
|---|---|---|---|
| Ardour | **拍にまっすぐ**。テンポ T は拍 b に対し `T = T0 + k·b`。時間で見るとテンポは指数。 | [S1][S2][S3] | A |
| MuseScore | 「Normal: 変わる速さが始めから終わりまで同じ」と書く。拍か時間かは書いていない。実際は段（4 段などの報告）。 | [S11] A、[S12] C | A（文面）、C（段の数） |
| Logic Pro | 「Create Tempo Curve」で段（テンポイベント）を作る。細かさは Density（1 小節に 1〜32 個）。各段は、その間の平均テンポ（利用者の報告）。 | [S8] B、[S9] C、[S37] A | B |
| Pro Tools | 「Linear」は「選んだ時間の範囲で均等に」。拍か時間かは書いていない。細かさは Tempo Edit Density。 | [S5] A、[S10] B | A（文面）、B |
| Cubase | 新しい点を「Jump」（すぐ変わる）か「Ramp」（前の点からだんだん）にできる。カーブの形は書いていない。 | [S6] | B（古い版） |
| REAPER | 「Gradually transition tempo to next marker」の箱と、テンポ線の点の形「linear」がある。どちらにまっすぐかは見つからなかった。 | [S13] | C |
| Bitwig、Ableton Live、Studio One、FL Studio | テンポの自動化はあるが、点の間が拍か時間か、公式の文書に書いてない。 | [S14][S15][S27] | 一次情報なし |

- Ableton は「テンポもほかと同じオートメーションの 1 つ」と書く（A）。
- FL Studio の「点の間はまっすぐ」はまとめサイトの話（C）。

食いちがい: Pro Tools の「時間の範囲で均等に」は時間にまっすぐと読めるが、文面が曖昧。確かめられていない。

### 1-2. Ardour のソース（A）

- `tempo.cc` の `TempoPoint::compute_omega_from_quarter_duration` [S1]:
  `_omega = ((1.0/end_scpqn) - (1.0/superclocks_per_quarter_note())) / DoubleableBeats(quarter_duration).to_double()`
  - `scpqn` は 4 分音符 1 つの長さ（クロック数）なので、1/scpqn はテンポ。
  - つまり ω = (終わりのテンポ − 始めのテンポ) ÷ 拍数。拍あたりのテンポの増え方。
  - 始めと終わりが完全に同じなら `_omega = 0.0`。
- 拍 → 時間 `superclock_at` [S1]: `log_expr = scpqn * _omega * Δ拍`、`r = _sclock + llrint(log1p(log_expr) / _omega)`。`log_expr < -1` なら中止（テンポが 0 以下になる）。
- 時間 → 拍 `quarters_at_superclock` [S1]: `b = (exp(_omega * (sc - _sclock)) - 1) / (scpqn * _omega)`。
- その時のテンポ [S1]: `scpqn * exp(-_omega * (sc - sclock))`。
- 「だんだん」かどうか [S2]: `actually_ramped() = ramped() && _omega != 0`。0 かどうかを**ちょうど 0 か**で見ている。ω が 0 でなくてとても小さい時は、`exp(...) - 1`（`expm1` でない）が桁落ちする（ソースを読んだ上での私の推論、C）。
- `continuing`（[S2] のコメント）: 終わりのテンポを、自分の `end` でなく次のテンポ点の値にする。
- 作者の書類 [S3]（GPL-3、Rivaud、2018）は、時間にまっすぐ（テンポが t に対し一次）と拍にまっすぐを両方書いたうえで、「人は絶対の時間より拍を追うので、テンポは拍にまっすぐにする」と選んだ。

### 1-3. 式の導きかた

記号: T = テンポ（拍/分）、b = 拍、t = 時間（分）、T0 = 始めのテンポ、T1 = 終わりのテンポ、Δb = だんだんの長さ（拍）。入力は [S3] の定義（A）。導きは私がやり直した。Ardour の式と一致する。

**(a) 拍にまっすぐ**
1. T(b) = T0 + k·b、k = (T1 − T0)/Δb とおく。
2. T = db/dt なので、dt = db / (T0 + k·b)。
3. 積分して、`t(b) = (1/k)·ln(1 + k·b/T0)`。
4. 逆にして、`b(t) = (T0/k)·(e^{k·t} − 1)`。
5. 時間で見たテンポは `T(t) = T0·e^{k·t}`（指数）。
6. だんだん全体の時間は `Δb·ln(T1/T0)/(T1 − T0)`。

**(b) 時間にまっすぐ**
1. T(t) = T0 + a·t。長さを拍 Δb で決めると、Δt = 2Δb/(T0 + T1)、a = (T1 − T0)/Δt。
2. `b(t) = T0·t + a·t²/2`。
3. `t(b) = (√(T0² + 2ab) − T0)/a = 2b / (√(T0² + 2ab) + T0)`（後ろの形は a → 0 でも `b/T0` になる）。
4. 拍で見たテンポは `T(b) = √(T0² + 2ab)`。

秒にするには ×60。ティックにするには ×480 拍。

**例（5 小節目 104 → 9 小節目 120、4/4、Δb = 16 拍 = 7680 ティック）。手計算なので、テストで確かめてください。**

| 量 | 拍にまっすぐ | 時間にまっすぐ |
|---|---|---|
| 7 小節目（b = 8）のテンポ | **112.00** | 約 112.29 |
| 7 小節目に着く時間 | 約 4.4465 秒 | 約 4.4387 秒 |
| だんだん全体の時間 | 約 8.5860 秒 | 約 8.5714 秒 |

差は 10 ms ほど。小さいが、拍にまっすぐでないと仕様の「112.00」にならない。

### 1-4. 数の計算で気をつける所と逃げかた（導きは C 寄りの設計案、入力は A の式）

- 0 で割る形になるのは、k（または x = k·b/T0）が 0 に近いとき。
- 逃げかた:
  - `t(b) = (b/T0)·L(x)`、`L(x) = log1p(x)/x`。
  - `b(t) = T0·t·E(y)`、`E(y) = expm1(y)/y`、y = k·t。
  - |x| か |y| が 1e-6 ほどより小さい時は級数にする。`L(x) ≈ 1 − x/2 + x²/3`、`E(y) ≈ 1 + y/2 + y²/6`。
  - k が 0 ちょうどなら `b/T0` と `T0·t`。
- 1 を足してから対数をとると桁落ちする。`log1p` を使う。作者の書類も同じ注意を書く [S3]。
- `x > -1`（テンポが 0 より大きい）でないとだめ。Ardour は `< -1` で中止する [S1]。T0、T1 は 0 より大きい値に限る。
- 区間ごとに始めの時間をためると、足し算の誤差がたまる。区間の始めの時間は、整数ティックから 1 つずつ計算しなおす。
- 時間 → 拍は、区間の終わりを越えたら次の区間の式に切りかえる（二分探索で区間を探す）。

### 1-5. SMF の書き出し

- テンポのイベントは `FF 51 03 tttttt`。tttttt は 24 ビットの符号なし整数（上位が先）、単位は「4 分音符あたりのマイクロ秒」。BPM = 60,000,000 ÷ tttttt。例: 120 BPM は 500000（`07 A1 20`）。ない時の既定は 500000 [S33] B。
- 範囲は 1〜16777215（私の計算で約 3.58 BPM 以上）。**「だんだん」を表すイベントはない**ので、段で表す。
- 他の DAW のきざみ:
  - **Logic**: Density で選ぶ。1 小節に 1〜32 個。1/8 より細かいのは「本当に要る時だけ」（処理が重くなる）[S8] B。iPad 版は Snap の値で段の数が決まる [S37] A。各段は区間の平均 [S9] C。始めと終わりの段は半分の大きさ（利用者の報告）C。
  - **Pro Tools**: Tempo Edit Density が、イベントを入れる間隔 [S10] B。
  - **Cubase**: Ramp は MIDI ファイルに出ない。「64 分音符よりずっと細かい段」で埋めると残った（利用者の報告）[S7] C。
  - **MuseScore**: 4 段などの粗い段（利用者の報告）。段の数をふやしてほしいという要望が出ている [S12] C。
  - REAPER は、段の数や間隔の記録が見つからなかった。
- 段を書くとき（私の案、C）: 区間の始めから終わりまでの本当の時間 Δt を式で出し、その区間のテンポを `Δb ÷ Δt`（平均）にする。こうすると、段の境い目の時間が、本当のだんだんとぴったり合う（ずれは段の中だけ）。細かさは、1 段のテンポの変わりが 0.05 BPM 以下か、120 ティック（32 分音符 2 つ分）の短いほう、を目安にする。

## 2. オートメーションのくわしい中身

### 2-1. 書くときの形

| DAW | Touch | Latch | Write | 止めたとき | 確かさ |
|---|---|---|---|---|---|
| Ardour | 動かした所だけ上書き | 最後の値で、再生が止まるまで | 書きつづける。**止めると Touch に自動で変わる**。保存時も Touch（空なら Off）、読みこむとき Write は Off になる | Write → Touch（A）。Latch は止めると終わり。コードでは再生中の `stop_touch` を Latch が早く返し、触っている状態を保つ。トグルは Touch でも Latch の動き | A [S16][S17][S18] |
| Logic | 離すと元の値に戻る。戻る時間は「Ramp Time」 | 離したあとの値で既存を置きかえ、止めて終わる | 通った所を全部消す | 設定「'Write' Mode Changes To」で、書いたあとの状態を選ぶ | B（古い版）[S24][S25] |
| Cubase | 離すと元の値へ戻る（Return Time） | **Auto-Latch**: 離しても書きつづけ、止めるか Write を切るまで | **Cross-Over**: 離したあと、既存の線にまたがった所で書きおわる | どのモードも、Write を切る、止める、早送り ・ 巻き戻し、ルーラーのクリックで書きおわる | A（検索の抜粋で確認、全文は読めていない）[S28] |
| Studio One（Fender Studio Pro） | 離すと読みに戻る | 触れたら、再生が止まるまで書く。再開時は触るまで読み | 既存は読まず、上書き | Latch は止めると終わり | A [S27] |
| Bitwig | 動かしている間だけ書き、離すと止まり、前の値は残る | 動かしはじめから、止めるまで | 動かしたかどうかによらず、始めから止めるまで書き、通った点を上書き（いちばん壊しやすい） | 止めたあと、曲線が最適化される | A [S15] |
| Ableton Live | Session で**マウス**を使うと、離した瞬間に前の値へ戻る | Session で MIDI のつまみ ・ フェーダーを使うと、クリップかループの終わりまで最後の値。最初の録音は常に Latch の動き | 書いている間、同じパラメータを上書き | 録音ボタンで「録音だけ止めて再生は続ける」ことも、停止ボタンもできる | A [S14] |
| REAPER | 離すと止まる | 離した値で書きつづける | 上書き | 「automation recording return speed」が、書いている線と既存の線のあいだをつなぐ時間（短いと急に戻る） | C [S31][S32] |

- REAPER の既定は「Trim/Read」で、フェーダーは既存の線を相対でずらす。「Latch Preview」（5.22 以降）は、線を変えずに値を試せる（C）。
- Ableton の「再び有効にする」前の値の動きは、2-2 で。

### 2-2. 手で動かして「休む」形

**Ableton Live（Live 12 マニュアル 26.4、A）[S14]**
- 休む操作: 「録音していない時に、パラメータの値を動かす」。
- 休んでいる間: そのパラメータの自動化の表示（LED）が灰色になる。値は手で決めた値のまま（自動の線は効かない）。
- 戻す操作:
  - コントロールバーの **Re-Enable Automation** ボタン（休んでいる物があると点く）。押すと**全部**戻る。
  - 1 つのパラメータだけは、そのパラメータの右クリックメニューの「Re-Enable Automation」。
  - Session View では、自動化の入ったクリップをもう一度起動しても戻る。
- ヘルプ記事 [S38]: ボタンが、再生を始めた時や曲を開いた時にひとりで点くことがある。原因の 1 つは、MIDI の割りあてで、機器からフィードバックが戻る事（A、要約で確認）。
- **休みが曲に保存されるか、取り消しの対象か**: マニュアルにない（一次情報は見つからなかった）。前の記事から、保存されないらしい（C、推論。曲を開いた時に「休み」が生まれる話であって、保存の話ではない）。

**Bitwig（ユーザーガイド latest、A）[S15]**
- 休む操作: 自動化されたパラメータをつかんで調整する（「一時的に値を上書きする」）。
- 見え方: 音量の目印が青から緑になる。**Restore Automation Control** ボタンが緑になる（「armed」）。
- 戻す操作: そのボタンを押すと**全部のパラメータ**に自動化の制御が戻る。1 つだけ戻す操作は、ガイドにない。
- 保存と取り消し: 書いていない。

**そのほか**
- Cubase は自動では休まない。人が「Suspend Read」（読みをパラメータごとに止める）を押す [S30] B（古い版、抜粋）。
- Ardour は、レーンごとに Manual（手の値が優先）と Play（フェーダーが手の入力を受けつけない）を人が切りかえる [S16] A。
- 一次情報では、「休み」の取り消し履歴を書いた DAW は見つからなかった。

### 2-3. 書いた点の間引きかた

| DAW | 取りかた | 間引きかた | 確かさ |
|---|---|---|---|
| Ardour | `automation_interval_msecs` 既定 **30 ms**（マニュアルの範囲は 1〜1000 ms） | `thin(thinning_factor)`: となりの 3 点の三角形の面積（時間はサンプル、値は 0..1 に正規化）が factor 未満なら真ん中の点を捨てる。既定 **20.0**。ゲインだけ factor に 0.7071 をかける（昔の値の範囲の名残り）。factor が 0 かトグルならしない。書き終わった時（`write_pass_finished`）に、書いた物があれば `thin` | A [S19][S20][S21][S23] |
| Cubase | 触った点 | Reduction Level（0〜100 %）。0 % は同じ値のくり返しの点だけ消す。上げると滑らかに | B（古い版）[S29] |
| Bitwig | 録音中 | 止めたあと、「曲線が最適化される」。ペンで描いた線は「点が最小で形を保つ」 | A [S15] |
| Ableton Live | 録音中は自動で間引くとは書いていない | 手で「Simplify Envelope」（最適な点の数にする） | A [S14] |
| REAPER | 録音と手描きで、「Reduce envelope point data when recording or drawing automation」の設定が、いらない点を消す | あとから「Envelope: Reduce number of points」（スライダー） | C [S31] |

- Ardour の数を、私が当てはめてみた値（C、推論）: 48 kHz で 30 ms は 1440 サンプル。となりの点が前後 1440 サンプルだと、面積 20 は、直線からのずれが値の範囲の約 1〜3 %（面積の式が外積か、その半分かで変わる。ソースの式の細部は未確認）。
- **Ramer–Douglas–Peucker**（RDP）は、線を再帰で分け、許す差（tolerance）をこえる点だけ残す [S34] B。点が 1000 個なら平均 n log n。meloet に使うなら、横軸（時間）と縦軸（値）で単位が違うので、**縦の差**を許す値にし、値の範囲で正規化する（C、私の案）。

### 2-4. 線の上の値の出しかた

**Ardour（A）[S18][S19][S22]**
- 音量（ゲイン、センド、Insert のリターン）の既定は「Exponential」（フェーダー用）。Trim と MainOutVolume は「Logarithmic」。パンは Linear。
- Exponential の中身（`interpolate_gain`）: 2 つの点のゲインを**フェーダーの目盛の位置**に直し、位置でまっすぐに結び、ゲインに戻す。
  - 目盛の位置: `position = ((6·log2(g) + 192) / 198)^8`。逆は `g = exp(((position^(1/8) · 198) − 192) / 6 · ln 2)`。ゲイン g は、上限（既定 2.0、約 +6 dB）で割ってから使う。
  - 0 dB（g = 1）は、位置で約 0.78（私の計算）。
  - とても近い 2 点は、そのまま後ろの値（`TINY_NUMBER` 以下の差は省く）。
- Logarithmic は `from · (to/from)^fraction`。つまり **dB でまっすぐ**。
- Linear は `from + fraction·(to − from)`（値そのままでまっすぐ）。
- 書いている最中は Exp と Log のレーンを Linear に切りかえる（検索の抜粋で確認した、C 寄りの A）。
- 利用者の返事（B 以下）: フェーダーのレーンを Linear に切りかえられる。そのとき 0 dB は真ん中あたり。

**ほかの DAW**: 公式の文書で、点の間の補間の空間（dB か、ゲインか、フェーダーの目盛か）を書いた物は見つからなかった。
- REAPER: ボリュームのフェーダーの dB 範囲（既定 −72〜+24 dB）と、変わる速さの「shape」値が設定にある（Cockos wiki、要約で確認）[S39] B。
- Bitwig: 点と点の間は ALT ドラッグでカーブにでき、ALT ダブルクリックで線形に戻る [S15] A。

### 2-5. 書いた値の位置と、出口の遅れ

- **Ardour（A）[S17][S36]**: 値は `transport_sample()`（いまの再生位置）を `actually_set_value` で `timepos_t(pos)` に直して書く。この関数の中で位置をずらす所はない。ただしマニュアルは「時計は、スピーカーで聞こえている音の位置を示す」と書くので [S36]、この `transport_sample` が聞こえている位置なのか、エンジンの先の位置なのか、読んだ範囲ではわからなかった。
- **Logic（B、古い版）[S26]**: 自動化の全部を、ティック（正負）でずらす「Automation Snap Offset」がある。理由は「音の機器の遅れ、CPU が重い時、プラグインの遅れの補正」。人が自分で決める。
- **Pro Tools**: 書いた自動化が、システムの遅れぶん遅れる、という利用者の話（約 183〜207 ms と測った）。遅れの補正は書き込みを直さないという返事、直し方は書いたあとに Nudge で動かす（C）[S35]。
- Cubase、Bitwig、Ableton、REAPER: 書き込み位置と遅れの補正の文書は見つからなかった。

## 3. meloet への当てはめ（C、推論）

**テンポ**
1. 仕様書の例は拍にまっすぐで合うので、「だんだん」は拍にまっすぐ（Ardour と同じ）。時間にまっすぐは入れなくてよい。
2. データの形: テンポ点（ティック、BPM、「次の点までだんだん」の旗）。終わりのテンポは次の点の値。旗が立たないか最後の点なら一定。Ardour の `continuing` と同じ考え。
3. core の中に、区間ごとに「始めのテンポ T0、傾き k、始めの時間」を持つ純粋な関数を置く（`b ↔ t`）。`log1p`/`expm1` と級数で 0 に近い k を逃がし、区間の始めの時間は整数ティックから 1 つずつ出す。単体テストに「7 小節目が 112.00」「k = 0 と近い k がなめらかにつながる」「往復（ティック → 時間 → ティック）の誤差」を入れる。
4. 音のスレッドでは、1 ブロックごとにティックと時間を式で直す。式は log と exp だけで、メモリを取りにいかない。テンポの表は、`Arc` などで丸ごと差しかえる形が合うと思う（実装は棟梁に）。
5. SMF は、段にする。区間の平均テンポ（`Δb/Δt`）、ティック 480 のまま。段は「0.05 BPM 以下の変わり」か「120 ティック」の短いほう。読みこみで、細かい段をだんだんに戻すかは別に決める。

**オートメーション**
1. まず Touch と Latch。Write は壊しやすい（Ardour は止めると Touch にし、保存でも Touch に落とす [S18]）ので、入れるなら「止めると Touch」の形で。
2. 離したときに元の線へ戻る速さ（Logic、Cubase、REAPER にある）は、速い値（20 ms 級）から始めるのがよい。meloet の「値は 20 ms でなめらかに」の決まりと合う。
3. 「休む」形（Live、Bitwig）は、パラメータごとに「休み」の旗を持つ形が合う。ボタンで全部戻す。さらに 1 つずつ戻せると親切（Live はできる）。保存と取り消しは文書がないので、meloet で決める。私の案: 休みは曲に保存せず（曲を開くとすべて自動が効く）、取り消しの対象にもしない。書いた点の変更は、1 回さわって離す（または 1 回の書きパス）で 1 手。
4. 点の間引きは、離したときか止めたとき（`write_pass_finished` 相当）。取りかたは 1 コマ 1 値まで（meloet の「1 コマに 1 つ」と合う。Ardour は 30 ms）。間引きは、縦の差（値の範囲の 0.5〜1 % ほど）の RDP でよいと思うが、閾値は測って決める。
5. 線の間の値: 音量は、フェーダーの目盛の空間でまっすぐ（Ardour と同じ。画面のフェーダーの動きと線の見え方が合う）。センドと Trim は dB でまっすぐ。パンとプラグインの正規化値は値そのままでまっすぐ。
6. 書く位置: 手を動かしたその瞬間の「耳に届いている位置」に置く（録音の鍵と同じ決まり）。手は、聞こえている音に合わせて動くので、これが耳に合う。DAW の文書には、同じことをしている物は見つからなかった（Logic は人にオフセットを任せ、Ardour は `transport_sample` のまま。ただし Ardour の `transport_sample` が耳に届く位置かは未確認）。

## 出どころの一覧

| 番号 | URL | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/temporal/tempo.cc | `compute_omega_*`、`superclock_at`（log1p）、`quarters_at_superclock`（exp）、テンポの式（master、版の目印なし。要約を通して読んだ） | A | 2026-11-20 |
| S2 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/temporal/temporal/tempo.h | `actually_ramped() = ramped() && _omega != 0`、`continuing` のコメント | A | 2026-11-20 |
| S3 | https://raw.githubusercontent.com/Ardour/ardour/master/doc/tempo.tex | 拍にまっすぐ、時間にまっすぐの式、ω の定義、数値の注意（Rivaud、2018、GPL-3） | A | 2026-11-20 |
| S4 | https://manual.ardour.org/ardours-interface/rulers-and-markers/tempo/ | 「Ramp to Next」「Continue」、だんだんの線は斜め | A | 2026-11-20 |
| S5 | https://apps.avid.com/proToolsFirstHelp/version12.3/enu/Pro%20Tools%20First%20Help/Arr1.conductors.31.44.html | Pro Tools の Linear は「時間の範囲で均等に」（直接は読めず、検索の抜粋） | A（抜粋、古い版は 1 段下げ → B） | 2026-11-20 |
| S6 | https://steinberg.help/nuendo/v8/en/cubase_nuendo/topics/editing_tempo_and_signature/editing_tempo_tempo_track_editor_toolbar_r.html ほか | Cubase の Jump と Ramp（抜粋） | B（古い版） | 2026-11-20 |
| S7 | https://forums.steinberg.net/t/midi-files-and-gradual-tempo-changes-eg-ritardandi/672329 | Cubase の Ramp は MIDI ファイルに出ない（利用者） | C | 2026-11-20 |
| S8 | https://help.apple.com/logicpro/mac/9.1.6/en/logicpro/usermanual/chapter_33_section_5.html と section_6 | テンポカーブは多数のテンポイベント、Density（1/1〜1/32）、1/8 より細かいのは必要な時だけ | B（古い版） | 2026-11-20 |
| S9 | https://forums.steinberg.net/t/problem-with-syncing-gradual-tempo-changes-created-in-dorico-and-logic-pro/921894 | Logic は各段が区間の平均、最初と最後の段は半分（利用者） | C | 2026-11-20 |
| S10 | https://www.soundonsound.com/techniques/super-conductors（ほか検索の抜粋） | Pro Tools の Tempo Edit Density | B | 2026-11-20 |
| S11 | https://handbook.musescore.org/text/tempo-markings | 「Normal: 変わる速さが同じ」、Amount（目標テンポの %） | A | 2026-11-20 |
| S12 | https://musescore.org/en/node/339747 ほか（検索の抜粋） | MuseScore の rit./accel. は 4 段（利用者） | C | 2026-11-20 |
| S13 | https://forum.cockos.com/showthread.php?t=206963 ほか（ProRec、whyreaper） | REAPER の「Gradually transition tempo」の箱、点の形 linear | C | 2026-11-20 |
| S14 | https://www.ableton.com/en/live-manual/12/automation/ | Live 12 の 26.3 上書き、26.4 Overriding Automation、26.9.1 テンポ、Simplify Envelope | A | 2026-11-20 |
| S15 | https://bitwig.com/userguide/latest/automation | Latch・Touch・Write、上書きと Restore Automation Control、最適化、ALT ドラッグ | A | 2026-11-20 |
| S16 | https://manual.ardour.org/mixing/automation/automation-states/ | Manual・Play・Write・Touch・Latch、Write は止めると Touch | A | 2026-11-20 |
| S17 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/automation_control.cc | `start_touch`、`stop_touch`（Latch は再生中に続ける）、`actually_set_value` が `transport_sample` を使う | A | 2026-11-20 |
| S18 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/automation_list.cc | 既定の補間（ゲイン Exponential、Trim Logarithmic）、Write の保存は Touch、読みこみは Write → Off | A | 2026-11-20 |
| S19 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/evoral/ControlList.cc | `thin`（面積、0.7071）、補間の呼び出し、書きパス | A | 2026-11-20 |
| S20 | https://ardour.org/files/doxygen/classEvoral_1_1ControlList.html | `thin`（時間はサンプル、値は 0..1、既定 20） | A | 2026-11-20 |
| S21 | https://ardour.org/files/doxygen/rc__configuration__vars_8inc_8h_source.html | `automation_interval_msecs` 30、`automation_thinning_factor` 20.0（9.10） | A | 2026-11-20 |
| S22 | https://ardour.org/files/doxygen/control__math_8h_source.html | `gain_to_position`、`position_to_gain`、`interpolate_gain`、補間の式（9.10） | A | 2026-11-20 |
| S23 | https://manual.ardour.org/preferences-and-session-properties/preferences-dialog/ | 間隔 1〜1000 ms、thinning 0〜1000（要約。ページは古い） | A（古い → B） | 2026-11-20 |
| S24 | https://help.apple.com/logicpro/mac/9.1.6/en/logicpro/usermanual/chapter_28_section_2.html | Logic の Read・Touch・Latch・Write、Ramp Time | B（古い版） | 2026-11-20 |
| S25 | https://help.apple.com/logicpro/mac/9.1.6/en/logicpro/usermanual/chapter_44_section_9.html | Snap Offset（ティック）、Ramp Time、「Write」Mode Changes To | B（古い版） | 2026-11-20 |
| S26 | https://help.apple.com/logicpro/mac/10.4.5/en.lproj/lgcpb1a5eb20.html | 自動化のオフセットは、機器の遅れの補正（全トラック、ティック、正負） | B（古い版） | 2026-11-20 |
| S27 | https://fenderstudiopromanual.fender.com/en/Content/Automation_Topics/Automation_Modes.htm | Studio One（Fender Studio Pro）の Off・Read・Touch・Latch・Write | A | 2026-11-20 |
| S28 | https://www.steinberg.help/r/cubase-pro/14.0/en/cubase_nuendo/topics/automation/automation_automation_modes_c.html | Touch・Auto-Latch・Cross-Over、共通の書きおわり（検索の抜粋のみ。本文は読めず） | A（抜粋） | 2026-11-20 |
| S29 | https://archive.steinberg.help/nuendo/v12/en/cubase_nuendo/topics/automation/automation_automation_preferences_r.html ほか | Reduction Level（0 % は同じ値だけ消す）、Return Time（抜粋） | B（古い版） | 2026-11-20 |
| S30 | https://archive.steinberg.help/nuendo/v8/en/cubase_nuendo/topics/automation/automation_suspend_read_c.html | Suspend Read（抜粋） | B（古い版） | 2026-11-20 |
| S31 | https://reaper.blog/2022/05/automation-transitions | 「Reduce envelope point data…」の設定、return speed | C | 2026-11-20 |
| S32 | https://publish.obsidian.md/arendleejessurun/Atlas/Music+production/Reaper/Automation+modes+in+Reaper ほか | REAPER の Trim/Read、Latch Preview（5.22+） | C | 2026-11-20 |
| S33 | https://midi.org/community/midi-specifications/set-tempo-command-ff-51-03-tttttt ほか | `FF 51 03 tttttt`（24 ビット、µs/4 分音符、既定 500000）。MIDI 協会の公式仕様の本文は読めなかった | B | 2026-11-20 |
| S34 | https://en.wikipedia.org/wiki/Ramer%E2%80%93Douglas%E2%80%93Peucker_algorithm | RDP。元の論文 Douglas & Peucker 1973、Cartographica 10(2):112–122（MaRDI の一覧で確認） | B | 2026-11-20 |
| S35 | https://www.puremix.com/blog/adjusting-for-delay-compensation-when-automating | Pro Tools で書いた自動化が約 183〜207 ms 遅れる、Nudge で直す | C | 2026-11-20 |
| S36 | https://manual.ardour.org/synchronization/latency-and-latency-compensation/ | 時計はスピーカーで聞こえる音の位置、read-ahead。自動化の話はなし | A | 2026-11-20 |
| S37 | https://support.apple.com/guide/logicpro-ipad/lpip91b10b32/ipados（検索の抜粋） | Logic iPad のテンポカーブ、Snap が段の数を決める | A（抜粋） | 2026-11-20 |
| S38 | https://help.ableton.com/hc/en-us/articles/209069409-Re-enable-Automation-keeps-lighting-up | Re-Enable ボタンがひとりで点く原因（直接は読めず、検索の抜粋） | A（抜粋） | 2026-11-20 |
| S39 | https://www.cockos.com/wiki/index.php/Preferences_Editing_Behavior | REAPER のフェーダーの dB 範囲（既定 −72〜+24 dB）と shape（検索の抜粋） | B | 2026-11-20 |

## わからなかったこと、追加で調べるとよいこと

- **REAPER、Cubase、Logic、Bitwig、Ableton、Studio One、FL Studio がだんだんを拍にまっすぐにしているか**、公式の文書では確かめられなかった。REAPER の公式ユーザーガイド PDF は大きすぎて（10 MB 超）読めず、Cockos wiki は転送でたどれなかった。実機で、5 → 9 小節目の 104 → 120 を作って SMF に書き出し、7 小節目のテンポ（または時間）を見るのが早い。
- **Ardour の `transport_sample()` が、耳に届く位置か、エンジンの先の位置か**。ソース全体と音の流れ（`Session::transport_sample` と出口の遅れ）を読む必要がある。
- **Ableton と Bitwig の「休み」の取り消しと保存**。実機で、休ませたあと、取り消し、保存して開きなおす、を確かめるのが早い。
- **Ardour の thinning の面積の式の細部**（外積か半分か）。`thin` の関数本体を行ごとに読む。
- **Cubase 14 の公式ページ全文**（Touch・Auto-Latch・Cross-Over、Reduction Level の範囲）。ページが大きく、要約でしか読めなかった。
- **LMMS のテンポの自動化**は調べていない。
- **MuseScore 4 の「Normal」が、実際にどの変数でまっすぐか**。ソース（`engraving` の段を作る所）を読む必要がある。
- **Studio One の Tempo Track の「ramp」の公式の文面**。PreSonus と Fender の移行で、旧マニュアルが見つからなかった。「Time-Linear / Beat-Linear」は、音声クリップの時間の置きかたの設定で、だんだんの形の話ではないらしい（B、Sound on Sound の説明から読んだ）。
- 保存先（依頼どおり）: `/srv/meloet-agent/meloet/docs/research/tempo-and-automation.md`（私は書けないので、棟梁が保存する）。
