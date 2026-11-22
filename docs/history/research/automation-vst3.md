# VST3 のオートメーションとテンポの渡しかた（パラメータの変化、ProcessContext、ブロックの分けかた）

調べた日 2026-11-20（meloet-research）。確かさは A・B・C。[S番号] は最後の表。
見かたの注意: Steinberg のヘッダや文書は、道具が引用を 125 字までに切るため、要約で読んだ所がある（要約は文書の言葉に沿っている）。JUCE・Ardour・Surge のソースは `master`（取得日の最新）を読んだ。JUCE は版ちがいも確かめた。

## 結論

1. **点の数と位置（A）**: ヘッダに点の数の上限は書いていない。点は `sampleOffset`（ブロックの頭からのサンプル数）の小さい順に並べる。点と点の間は「前の点から次の点まで、まっすぐ」。前のブロックの最後の点は「位置 −1」にあるものとして扱う。**位置 0 の点は、値が前と同じなら省いてよい**。傾きがずっと同じなら、ブロックの最後のサンプルの値の 1 点だけを送る。値がとぶ所は 2 点（古い値と、次のサンプルの新しい値）。点の間をどれだけ正確に読むかは、プラグインしだい（文書のとおり）。[S1, S2]
2. **JUCE の VST3 の包みは、1 ブロックの中の点の位置を使わず、各パラメータの「最後の点」の値だけを使う（A）**。6.1.6、8.0.12、`master`（2026-11 時点の最新は 9.0.6）の 3 つで同じ。例外は、MIDI のコントローラに当てた特別なパラメータ（`JUCE_VST3_EMULATE_MIDI_CC_WITH_PARAMETERS`）だけで、そこは全部の点を位置つきで使う。[S14, S15, S16, S17] Surge XT は JUCE で動き、パラメータを JUCE のアダプターから受ける。この PC の Surge XT がどの JUCE の版かは確かめていないが、3 つの版で同じなので、**ブロックの中の位置は効かず、ブロックの終わりの値が 1 ブロックぜんぶに効く**と見てよい（推論 C、根拠は A）。[S21]
3. **窓のつまみ（A）**: ホストが値を音の側に届ける道は、`process` の `inputParameterChanges` だけ。窓の側（コントローラ）には、ホストが**UI スレッドで** `IEditController::setParamNormalized` をくり返し呼んで知らせる。コントローラは、その値を `IComponentHandler` で返してはいけない。`outputParameterChanges` はプラグインが「自分で動かした値」（メーターなど）をホストに返す口で、ホストはそれをコントローラへ `setParamNormalized` で渡す。[S2, S6, S7] JUCE のプラグインは、再生中は `setParamNormalized` の値を音の側に入れない（A）。だから音の側へは必ず `process` で送る。[S14]
4. **`beginEdit` ・ `performEdit` ・ `endEdit`（A）**: 窓のなかの人の操作を、プラグインが UI スレッドでホストに知らせる 3 つ。ホストはこれで「さわり始めと終わり」を知ってオートメーションを録る。Steinberg の文書に「touch」という言葉はなく、**touch の作りかたはホストにまかされている**。Ardour は `beginEdit` を touch の始まり、`endEdit` を終わりに結びつけている（A）。[S2, S18, S20] REAPER・Bitwig・Cubase の一次情報は見つからなかった。
5. **ProcessContext のテンポ（A、決まりは無い）**: SDK に「ブロックの中でテンポがだんだん変わるとき、`tempo` に何を入れる」という決まりはない。「ブロックごとに、時間で変わりうる情報を渡す」とだけある。**Ardour は、ブロックの頭のテンポを入れ、平均はとらず、ブロックの中の変化は無視する（A）**。[S4, S18] Surge XT は、ブロックごとに `tempo` を 1 回読み、ブロックの中の位置は自分でその値で進める（A）。[S21]
6. **ブロックを分けること（A）**: SDK は「ループのとびで、ブロックを分けて正しい位置を渡すことはできるが、必須ではない。ブロックの大きさが決まっているホストは、気にしなくてよい」と書く。`numSamples` は呼ぶたびに変わってよい。Ardour は、VST3 では**オートメーションの点でもブロックを分けない**（ほかの形式は分ける）。[S4, S8, S20]

## 1. IParameterChanges と IParamValueQueue

| 事がら | 中身 | 確かさ |
|---|---|---|
| 1 パラメータ 1 キュー | 「あるパラメータの ID は IParameterChanges の中に 1 回だけ」。変わったパラメータのキューだけ入れる | A [S1, S2] |
| 点の並び | 「点は offset（ブロックの中の位置）の順に並べる」 | A [S2] |
| `sampleOffset` | ブロックの頭からのサンプル数。`getPoint` の説明は「offset」とだけ。意味は、クラスの説明の「ブロック位置 0 の点は省いてよい」から。0 〜 `numSamples`−1 の範囲と読む | A（要約）、範囲は C [S1] |
| 点の間 | 「各点は、前の点から、まっすぐな線の区間を決める」。ヘッダの例は、前に送った最後の点を位置 −1 に置き、最初の点との間を直線で読む | A [S1] |
| 点の省きかた | 位置 0 の点は省いてよい。傾きがずっと同じなら、ブロックの最後のサンプルの値だけ送る | A [S1] |
| とぶ所 | 古い値の点と、次のサンプルの新しい値の点の 2 点 | A [S1] |
| 正確さ | 「ホストは、サンプル単位で曲線を作りなおせる形で送る」「実現はプラグインの仕事。どれだけ時間をかけるかも、プラグインが選ぶ」 | A [S2] |
| 点の数の上限 | ヘッダにも文書にも決まりなし。SDK のホスト用の部品 `ParameterValueQueue` は、上限なし（最初の席が 5 で、足りなければ広がる）、位置順に差しこみ、同じ位置の点は値を上書きし、いつも `kResultTrue` を返す。パラメータの数も、席が足りなければ新しい物を取る（上限で失敗しない）。ただし、この部品は音のスレッドで使えるつくりではない（`new` や `emplace_back` をする） | A [S11] |
| Ardour | キューは `std::vector` で広がる。同じ位置の点は上書き。パラメータの席も足りなければ広がる（「最初の 1 回だけで RT 安全でない」と書いてある） | A [S19] |

- **点が 1 つだけのとき**（SDK の文と手もとの読みから。推論 C）:
  - 位置 0 に置く: 「−1 の前の値」から「0 の新しい値」への直線なので、実質ブロックの頭からその値になる（段のつまみ向き）。
  - ブロックの最後のサンプルに置く: ブロックぜんたいをかけてその値へ動く（なめらかな変化向き。SDK の「傾きがずっと同じなら最後の値だけ」と同じ形）。
  - 値の変わる位置 p に置く: SDK のとおりなら 2 点（p−1 に古い値、p に新しい値）。
- **ホストが上限を決めてよいか**: 決まりは見つからなかった（A で「禁止もなし、保証もなし」）。ただし文書は「ホストは、サンプル単位で曲線を作りなおせる形で送る」と書くので、点を減らすと、減らした分、作りなおせなくなる。Ardour も SDK の部品も上限をおいていない。**上限をおくのは、実装としては許されるが、行儀のよさは落ちる**（推論 C）。
- 手もとの `vst3` 0.3.0（SDK 3.8.0 の束縛）には `IParameterChanges` ・ `IParamValueQueue` がそのまま入っている（A）。[S12]

## 2. JUCE の包みと Surge XT

- **JUCE（A）**: `processParameterChanges` は、パラメータごとに `numPoints = getPointCount()` を読み、**`getPoint(numPoints - 1, ...)` の 1 点だけ**を取って、`setValueAndNotifyIfChanged(param, value)` に渡す。`offsetSamples` は使わない。同じ形が 6.1.6（`paramQueue->getPoint(numPoints - 1, offsetSamples, value)`）、8.0.12、`master`。[S14, S15, S16]
  - 例外: `JUCE_VST3_EMULATE_MIDI_CC_WITH_PARAMETERS` が立っていて、そのパラメータが MIDI のコントローラ用なら、**全部の点**を `addParameterChangeToMidiBuffer(offsetSamples, ...)` に渡す（MIDI の位置になる）。[S14, S15]
  - 出力側: `process` の終わりで、変わったパラメータごとに `queue->addPoint(0, value, ...)`。つまり出力の点はいつも位置 0。[S14]
  - 古い JUCE フォーラムにも「JUCE の VST3 は、ブロックの最後の値を使う」という書きこみがある（C）。[S25]
- **Surge XT（A、読んだ所だけ）**: `SurgeParamToJuceParamAdapter::setValue` が `setParameter01` を呼ぶ。`processBlock` の中に、ホストのパラメータ用のキューは見つからなかった（読めた範囲）。音は内部のブロック（`BLOCK_SIZE` ごと。値は別のファイルで、32 と記憶しているが未確認、C）で作る。MIDI はサンプルの位置どおり。CLAP の道も、パラメータは内部ブロックの頭にだけ入る。[S21] ⇒ JUCE が最後の点だけを渡すので、**Surge XT の VST3 では、ブロック内の位置は効かない**（推論 C、根拠は A）。
- 影響: JUCE 系のプラグインは、**最後の点の値がブロックぜんぶに効く**。なめらかな変化では、ブロックの頭の値ではなく、**ブロックの最後のサンプルの値**を最後の点にすると、SDK の「最後のサンプルの値」の作法と同じで、点の間を読むプラグインにも、最後の値だけ読むプラグインにも合う（推論 C）。

## 3. 音の側と窓の側の知らせかた

- **ホスト → 音の側**: 「パラメータの変化が音の側に届く道は `process` だけ」。ホストはキューを作って渡す（A）。[S2] ホストが窓を持たないパラメータ（`kCanAutomate` のない物）も、「ホストがコントローラから音の側へ運ぶ」と FAQ にある（A）。[S9]
- **ホスト → 窓の側（オートメーションの再生）**: 「ホストは、オートメーションが音の側に送られているとき、窓を更新する責任がある。UI スレッドで `setParamNormalized` をくり返し呼ぶ」（A）。[S2] `setParamNormalized` は「値と、それに関わる窓の部品だけ」を変え、**この値を `IComponentHandler` でホストに返してはいけない**。UI スレッド、つながっている間（A）。[S6] 「窓の側の更新は、音の側より低い頻度でよい。遅れの合わせはホストがする」（Steinberg 側の人と見られる投稿者、2019、B）。[S22]
- **窓のつまみを人が動かした（コントローラ → ホスト）**: `beginEdit` → `performEdit` → `endEdit`、UI スレッド。ホストは記録でき、**次の `process` の `inputParameterChanges` で音の側へ渡さねばならない**（B）。[S22] meloet の `Handler` は、UI スレッド以外の `performEdit` は取りこぼしてよい作り。投稿者も「UI スレッド以外の `performEdit` は無視」と書く（B）。[S22]
- **ホストから窓の側を動かすとき、JUCE のプラグインは再生中、音の側を更新しない（A）**: コードの文は「再生中でなければ AudioProcessor を更新する。さもないと、再生中に 2 本の値の流れができる」。[S14] だから、`kPlaying` を立てた `process` で値を送るのがよい。止まっているとき（`kPlaying` なし）は、`setParamNormalized` が JUCE の音の側にも入る（A、コードから）。
- **`outputParameterChanges`（A）**: `ProcessData` で「出力のパラメータ変化（任意）」。つまり**ポインタは空でもよい**（プラグインが空をたしかめる）。メーターなど「窓に知らせたい値」は、パラメータとして出し、`process` の中で `outputParameterChanges` に入れると、ホストが窓へ正しい時刻に返す（FAQ）。[S5, S7] Steinberg の `audiohost` の例は、この口を空のまま（B）。[S22] Ardour は、キューの**最後の点**だけ読み、控えの値を更新して、窓の更新を後で行う（A）。[S18] meloet は、いまの実装で `last_values()` を読み、窓へ回している。[S13]
- `restartComponent` は UI スレッドから。それ以外のスレッドで呼んできたら、ホストは UI スレッドに回してよい（FAQ、A）。[S9] `kParamValuesChanged` は「ホストが値を読みなおす」、`kParamTitlesChanged` は「名前、単位、はじめの値、段の数、旗を読みなおす」（A）。[S6]
- `IEditControllerHostEditing`（`beginEditFromHost` ・ `endEditFromHost`）が `vst3` 0.3.0 にあり、プラグインが持っていれば、ホストの側から動かす操作の始まりと終わりを知らせられる（A、存在のみ）。[S12, S22]

## 4. beginEdit ・ performEdit ・ endEdit の作法

- 文書（A）[S2]: つまみとスライダーは、マウスを押したとき `beginEdit`、動かしたとき `performEdit`、離したとき `endEdit`。**マウスのホイールは始まりも終わりもないので、プラグインはタイマーで区切り、ホストは `beginEdit` なしの `performEdit` も受ける**。ボタン、ラジオ、ポップアップ、文字の入力は、3 つを続けて呼ぶ。順番を守る。UI スレッド。「オートメーションの録音はホストの仕事」。
- **Ardour（A）** [S18, S20]:
  - `beginEdit` → 画面の知らせ `BeginGesture` → `Plugin::StartTouch`。`endEdit` → `EndTouch`。`PluginInsert::start_touch` ・ `end_touch` が、その時の `audible_sample()` でオートメーションの touch を始め、終える（遅れの引き算はまだ「ToDo」）。
  - `performEdit` は、値を控え、音のスレッドへ `_parameter_queue` で渡し、`parameter_changed_externally` を呼ぶ。
  - ホストが窓を動かすときは「GUI スレッド」で `setParamNormalized`。
  - オートメーションの状態（読む、touch など）は `setParamAutomationMode` でプラグインに知らせる。
- **REAPER ・ Bitwig ・ Cubase**: 公式の文書は見つからなかった。フォーラムに、Cubase が `endEdit` のあとに 0.0 を送ってくる（Latch で）という報告、REAPER は窓が閉じているとオートメーションを書かないことがある（「すべての通知を処理」の設定で直る）という報告がある（C）。[S24] 「touch」は、人がさわっている間だけ書き、離すと元のオートメーションに戻る動き、というのが各ホストのふつうの意味（C）。
- 「窓が制限した値」: プラグインが `setParamNormalized` の値を端などに丸めたら、`performEdit` で新しい値を返すべき、という Steinberg 側の人の投稿（B）。[S22]

## 5. ProcessContext

手もとの束縛 [S12] A と、ヘッダの文 [S3, S4] A:

| 欄 | 意味 | 旗 | meloet のいま |
|---|---|---|---|
| `state` | 旗の組みあわせ | — | 5 つの Valid と再生など |
| `sampleRate` | いつも有効 | — | 入れている |
| `projectTimeSamples` | 「プロジェクトの時間（サンプル）」。いつも有効 | — | `position_frames`（コメントは「いまのテンポでの」） |
| `systemTime` | ナノ秒 | `kSystemTimeValid` | 入れていない |
| `continousTimeSamples` | 「プロジェクトの時間、ループなし」 | `kContTimeValid` | `frames_run` |
| `projectTimeMusic` | 4 分音符の数（1.0 = 4 分音符 1 つ） | `kProjectTimeMusicValid` | 入れている |
| `barPositionMusic` | 「いちばん最近の小節の頭」の位置（4 分音符の数） | `kBarPositionValid` | `bar_start_quarters` |
| `cycleStartMusic` ・ `cycleEndMusic` | ループの始めと終わり（4 分音符の数） | `kCycleValid`（値が有効）、`kCycleActive`（ループが入っている） | ループ中に入れている |
| `tempo` | BPM | `kTempoValid` | 入れている |
| `timeSigNumerator` ・ `timeSigDenominator` | 例: 3/4 なら 3 と 4 | `kTimeSigValid` | 入れている（0 なら 4） |
| `samplesToNextClock` | MIDI クロック（4 分音符 24 個）。マイナスもある | `kClockValid` | 入れていない |
| 再生・録音 | 「いま再生中」「いま録音中」 | `kPlaying`、`kRecording` | 入れている |

- **要る欄の申告（A）**: プラグインは `IProcessContextRequirements`（SDK 3.7.0 から。Change History に名前だけ）で、要る欄を一度だけ（initialize のあと、setActive の前）申告する。旗は `kNeedTempo`、`kNeedTimeSignature`、`kNeedProjectTimeMusic`、`kNeedBarPositionMusic`、`kNeedCycleMusic`、`kNeedContinousTimeSamples`、`kNeedSystemTime`、`kNeedSamplesToNextClock`、`kNeedTransportState` ほか。3.7 より前の SDK で作った物は、正確さの低い情報を受ける。[S5, S10, S12] meloet のコードに `IProcessContextRequirements` を呼ぶ所は見つからなかった（Grep）。[S13]
- **ブロックの中でテンポがだんだん変わるとき**:
  - SDK（A）: 決まりなし。「ブロックごとの時間情報で、時間で変わりうる」。[S4]
  - Ardour（A）: ブロックの頭の位置 `start` の `metric_at(start)` から `tempo` と `projectTimeMusic` を作る。「ブロックの中の変化は無視」「平均なし」。MIDI 点の `ppqPosition` も、ブロックの頭の値を共有する。[S18]
  - Surge XT（A）: `cp.bpm` と `ppqPosition` をブロックの頭で読み、あとは内部ブロックごとに `BLOCK_SIZE * tempo / (60 * samplerate)` で位置を進める。つまり**ブロックの頭のテンポと位置から先を自分でたしざんする**。[S21]
  - JUCE のホスト側の話（フォーラム、C）: 「`tempo` と `ppqPosition` は現在のブロックの頭だけを表す。ブロックの中で急に変わったか、なめらかに変わったかは区別できない」。KVR にも、ドラムマシンの作者が「ホストもプラグインもブロックの中でテンポが変わらないと仮定している」と書く（2014）。[S25]
  - REAPER ・ Bitwig: 一次情報は見つからなかった。
- **JUCE の包みの読みかた（A）**: `kTempoValid` があるときだけ `tempo`、`kTimeSigValid` があるときだけ拍子、`kProjectTimeMusicValid` があるときだけ ppq、`kBarPositionValid` があるときだけ小節の頭、`kCycleValid` があるときだけループの点。`kCycleActive` でループ中かどうか。[S14] ⇒ 旗が立っていない欄は読まれない。
- **Ardour の細かい所（A のコード、判断は C）**: `barPositionMusic = (bbt.bars - 1) * 4`（コメントで `divisions_per_bar` ではないと書く）。**1 小節が 4 分音符 4 つの前提**で、3/4 や 6/8 ではずれるはず。meloet は `bar_start_quarters` をちゃんと持っているので、まねしない。また `systemTime` に `g_get_monotonic_time()` をそのまま入れる（この関数の単位はマイクロ秒と記憶、SDK はナノ秒。食いちがいの疑い、C）。`samplesToNextClock` は `#if 0` で入れていない。`kPlaying` は速度が 0 でないとき、止まっていても `tempo` ・ 拍子 ・ 小節は入れる。[S18]
- 拍子の分母: 例の「3/4 なら 3 と 4」のとおり。`projectTimeMusic` と `barPositionMusic` の単位はいつも 4 分音符。6/8 なら 6 と 8 を入れ、1 小節は 4 分音符 3 つ（4 × 6 / 8、計算のみ C）。

## 6. ブロックを分けるか

- SDK（A）: ループなどのとびでは、ホストがブロックを分けて「どのブロックの中も、プロジェクトの時間が正しい」ようにできるが、必須ではない。「ブロックの大きさが決まっているホストは、この問題を無視してよい。次のブロックでまた時間が正しくなる」。[S4]
- `numSamples`（A）: 呼ぶたびに変わってよい。最大の大きさを超えてはいけない。最大を変えるには、止めて有効にしなおす。[S8] 音を処理しないときの「流し」（flush）は、バス数 0 などで `process` を呼ぶ。[S5, S8]
- **Ardour（A）** [S20]:
  - `PluginInsert::automation_run` は、ループの終わりでだけ分ける。
  - `automate_and_run` は、オートメーションの点でブロックを分けて `connect_and_run` を何度も呼ぶが、**VST3 のプラグインと、固定の大きさを求める物は、分けない**（`no_split_cycle`）。
  - VST3 は分けるかわりに、ブロックの中の各オートメーションの点を `set_parameter(id, val, offset)` で位置つきで送り、サイクルの終わりの値も入れる。
  - テンポの変化でブロックを分ける所は、`PluginInsert` のコードには見つからなかった。
- JUCE の包みは最後の点だけ使う（上の 2）ので、点でブロックを分けても分けなくても、JUCE 系のプラグインには効果の差が出るのは「ブロックの終わりの値の入りかた」だけ（推論 C）。
- REAPER ・ Bitwig ・ Cubase が分けるかは、一次情報が見つからなかった。

## meloet への当てはめ（C、推論）

1. **点の数の上限の案**:
   - 手もとの実装は 1 パラメータ 16 点、1 ブロック 128 パラメータで、一杯になったら最後の点を上書きして、ブロックの終わりの値を守る（`com.rs`）。この形は SDK の文と合う（最後の点が大事）。16 のまま始めてよい。
   - オートメーションの線が折れ線なら、ブロックの中の折れ点 ＋ ブロックの終わりの 1 点で線を作りなおせる。ふつうの曲では、1 ブロック（256 フレームで約 5.3 ms）に折れ点は 0〜2 個。曲線（なめらかな線）なら、ブロックの中を数点（たとえば 4〜8 点）で折れ線にする。
   - **溢れたときは、途中の点を捨て、ブロックの終わりの点を残す**（いまの「最後を上書き」と同じ考え）。
   - 値が変わらないパラメータは送らない（SDK の「変わったキューだけ」）。ただし、再生を始めたとき、シークしたとき、ループで戻ったときは、オートメーションのある全パラメータに、位置 0 の点を 1 つ送ると、まちがいがない。
2. **ブロックの中の位置の入れかた**:
   - 位置は 0 〜 `numSamples`−1 のフレーム。小さい順。
   - なめらかな変化は、最後の点を `numSamples`−1 の位置に、そのサンプルの値で入れる（JUCE 系にも、点の間を読む物にも合う）。
   - 段のつまみ（`stepCount` が 1 以上、オン・オフ）は、変わる位置 p に新しい値、p が 0 より後なら p−1 に古い値を入れる。JUCE 系ではブロックの終わりの値だけ効くので、段の位置はブロックの区切りになる。
   - 音量とパンは、meloet の最初から入っているミキサーが、同じ線から 20 ms のなめらかさで自分で処理する（CLAUDE.md の決まり）ので、この話はプラグインのパラメータだけ。
3. **窓のつまみの動かしかた**:
   - いまの `to_controller` の輪（1024 個、`hear_audio` が主スレッドで `setParamNormalized`）は、SDK の作法と同じ形で、そのまま使える。
   - ただし、**1 点ごとではなく、1 ブロックにパラメータごと最後の値 1 つだけ**を輪に入れると、輪が溢れない（輪の `push` は溢れると黙って捨てる）。
   - 主スレッドでは、同じ ID の古い値を捨てて最新だけ `setParamNormalized` するのがよい（窓の描きなおしが 60 Hz をこえない）。
   - `setParamNormalized` の値は、コントローラから `performEdit` で返ってこない（SDK の決まり）。返ってきたら無視する（`Handler` はいまも、窓の人の操作と区別できない形なので、ホストから呼んだ最中に来た `performEdit` を無視する印を持つと安全。ここは C）。
   - 人が窓のつまみをさわっている間（`beginEdit` ～ `endEdit`）は、その ID のオートメーションの点を送らず、人の値を送る（touch の動き。Ardour の考えかたと同じ。C）。離したあと、元の線に戻すか、離した値で線を書きなおすかは、画面の仕様しだい。
   - 音は、必ず `process` の `inputParameterChanges` で送る。JUCE 系は、再生中は `setParamNormalized` が音の側に入らない。
4. **ProcessContext の入れかた**:
   - **`tempo` は、ブロックの頭のテンポ**（Ardour と同じ。Surge XT は頭の値から先を自分でたす）。`projectTimeMusic` は、ブロックの頭のぴったりの位置（テンポの変化を積みあげた値）。
   - ブロックの終わりの位置と次のブロックの頭の位置をなめらかにつなぐ（連続させる）には、「ブロックの平均のテンポ」を入れる手もある。SDK は禁止していないが、Ardour などはしていない。まず頭のテンポでよく、にごりが聞こえたら試す（C）。
   - `barPositionMusic` は、拍子の変更を足した、その小節の頭の位置（Ardour のように「小節 − 1 に 4 をかける」はしない）。拍子は分子と分母をそのまま（6/8 なら 6 と 8）。
   - `projectTimeSamples` は、テンポの変更があっても、**実際のサンプルの位置**（いまの `position_frames` のコメントは「いまのテンポでの」。テンポが変わる曲では、この数え方を見なおす。C）。
   - `IProcessContextRequirements` を、`setActive` の前に 1 度聞くのを足す価値がある（要る旗だけ入れれば足りる。要らない物の計算が減る）。`systemTime` は、入れるなら単位はナノ秒。
   - 止まっているときも、`tempo`、拍子、位置は入れ、`kPlaying` だけ外す（Ardour と同じ）。
5. **ブロックを分けるべきか**: **分けない**でよい（C）。
   - 理由: SDK が必須としていない（A）。Ardour は VST3 で分けない（A）。JUCE 系は最後の点しか使わないので、分けるとかえって「そのブロックの最後」が早く来て、値の入りが細かくなるだけ。
   - 分けると、プラグインとの往復（親と子で p50 8〜13 µs、20-2）がその分ふえ、時間切れの見はりにも効く。
   - ループの折り返しがブロックの途中に来る所は、エンジンがいま音のブロックを折り返しで分けているか分けていないかを、手もとのソースで見ていない（未確認）。**ここだけは、調べてから決める**。分けるなら、分けたブロックのどちらにも ProcessContext が正しい位置になる。分けないなら、折り返しの 1 ブロックだけ `projectTimeMusic` が飛ぶことになり、SDK はそれを許す（A）。

## 出どころの一覧

| 番号 | URL ・ 場所 | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstparameterchanges.h | `IParamValueQueue` ・ `IParameterChanges` の説明（直線、位置 0 を省く、最後のサンプル、ジャンプは 2 点）。`[released: 3.0.0]`、`[host imp]` | A（引用が 125 字までのため要約で読んだ） | 2026-11-20 |
| S2 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Parameters+Automation/Index.html | 1 パラメータ 1 キュー、位置順、サンプル単位で作りなおせる、窓の更新は UI スレッドで `setParamNormalized`、`beginEdit` ・ `performEdit` ・ `endEdit` の作法 | A | 2026-11-20 |
| S3 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstprocesscontext.h | `ProcessContext` の欄と旗の説明 | A | 2026-11-20 |
| S4 | https://steinbergmedia.github.io/vst3_doc/vstinterfaces/structSteinberg_1_1Vst_1_1ProcessContext.html | 時間情報は変わりうる、ブロックを分けるのは必須でない、各欄の説明 | A | 2026-11-20 |
| S5 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstaudioprocessor.h | `ProcessData`（出力のパラメータ変化は任意、`processContext` は「任意だが歓迎」）、`IProcessContextRequirements` の旗 | A | 2026-11-20 |
| S6 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivsteditcontroller.h | `setParamNormalized`（値と窓だけ。ホストに返さない）、`beginEdit` 等は UI スレッド、`kParamValuesChanged` ・ `kParamTitlesChanged` | A | 2026-11-20 |
| S7 | https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Communication.html | メーターなどは `outputParameterChanges`、ホストが窓へ返す | A | 2026-11-20 |
| S8 | https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Processing.html | `numSamples` は呼ぶたびに変わってよい、流し（flush）の呼びかた | A | 2026-11-20 |
| S9 | https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Hosting.html | 自動にできないパラメータもホストが運ぶ、`restartComponent` は UI スレッド | A | 2026-11-20 |
| S10 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Change+History/Index.html | `IProcessContextRequirements` は 3.7.0 | A | 2026-11-20 |
| S11 | https://raw.githubusercontent.com/steinbergmedia/vst3_public_sdk/master/source/vst/hosting/parameterchanges.cpp | SDK のホスト用の部品: 点の上限なし、位置順に差しこみ、同じ位置は上書き、席は広がる | A | 2026-11-20 |
| S12 | `/home/meloet-agent/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/vst3-0.3.0/src/bindings.rs` | `ProcessContext`（14240 行）、`StatesAndFlags_`（14266〜14281）、`kNeed*`（12142〜12152）、`IEditControllerHostEditing`（8939） | A（手もとの実物） | 2026-11-20 |
| S13 | `/srv/meloet-agent/meloet/crates/meloet-plughost/src/formats/vst3.rs`（`context`、`process`、`hear_audio`）と `vst3/com.rs`（`ParamQueue`、`ParamChanges`、`Handler`）、`crates/meloet-plugin/src/ipc/mod.rs`（`Transport`）、`docs/architecture.md` 20-2 ・ 20-14 | meloet のいまの作り（16 点、128 パラメータ、`last_values`、旗） | A（手もとの実物） | 2026-11-20 |
| S14 | https://raw.githubusercontent.com/juce-framework/JUCE/master/modules/juce_audio_plugin_client/juce_audio_plugin_client_VST3.cpp | `processParameterChanges`: 最後の点だけ（`numPoints - 1`）、MIDI CC 用の特別枝は全点、出力は位置 0、`Param::setNormalized` は再生中は音の側を更新しない、`getPosition` は旗があるときだけ値を読む | A | 2026-11-20 |
| S15 | https://raw.githubusercontent.com/juce-framework/JUCE/8.0.12/modules/juce_audio_plugin_client/juce_audio_plugin_client_VST3.cpp | 8.0.12 でも同じ | A | 2026-11-20 |
| S16 | https://raw.githubusercontent.com/juce-framework/JUCE/6.1.6/modules/juce_audio_plugin_client/VST3/juce_VST3_Wrapper.cpp | 6.1.6 でも最後の点 | A | 2026-11-20 |
| S17 | https://github.com/juce-framework/JUCE/releases | 最新は 9.0.6（19 Nov）。VST3 のパラメータの変更の記載なし | A | 2026-11-20 |
| S18 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/vst3_plugin.cc | `connect_and_run`: ブロックの頭のテンポ、`barPositionMusic` は `(bars-1)*4`、`beginEdit` → touch、`setParamNormalized` は GUI スレッド、出力は最後の点 | A | 2026-11-20 |
| S19 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/vst3_host.cc | 点とパラメータの席は広がる、同じ位置は上書き | A | 2026-11-20 |
| S20 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/plugin_insert.cc | VST3 はブロックを分けず、位置つきの点を送る。touch の始めと終わり | A | 2026-11-20 |
| S21 | https://raw.githubusercontent.com/surge-synthesizer/surge/main/src/surge-xt/SurgeSynthProcessor.cpp | `processBlockPlayhead`（ブロックごとに `bpm` ・ ppq）、`setValue` → `setParameter01`、MIDI だけサンプル位置どおり | A（読めた範囲のみ。ファイルの全部ではない） | 2026-11-20 |
| S22 | https://forums.steinberg.net/t/clarification-of-parameter-handling-in-vst-3/201914 、 https://forums.steinberg.net/t/question-clarification-regarding-parameter-interaction-when-implementing-a-plugin-host/974767 | Yvan（2019〜2024）と Arne Scheffler（2025）の答え。2 人とも Steinberg の人と見られるが、ページには明記なし | B | 2026-11-20 |
| S23 | https://forum.juce.com/t/sample-accurate-parameter-automation/50277?page=4 | JUCE のフォーラム。「最後の値だけ」を言う投稿は読めなかった（検索の要約にだけあった） | C | 2026-11-20 |
| S24 | https://forums.steinberg.net/t/cubase-sending-unwanted-0-0-parameter-value-after-endedit/697244 、 https://forum.juce.com/t/reaper-does-not-write-param-automation-if-gui-is-closed/25568 | Cubase の `endEdit` 後の 0.0、REAPER の「窓が閉じていると書かない」の報告 | C（検索の要約のみ） | 2026-11-20 |
| S25 | https://forum.juce.com/t/position-of-the-host-tempo-change/9318 、 https://www.kvraudio.com/forum/viewtopic.php?t=420867 | 「テンポと ppq はブロックの頭だけを表す」「ブロックの中でテンポは変わらないと仮定」 | C（古い、検索の要約のみ） | 2026-11-20 |

## わからなかったこと、追加で調べるとよいこと

- **REAPER ・ Bitwig ・ Cubase の touch ・ テンポのブロックの中の扱い**の一次情報（公式の文書やソース）は見つからなかった。確かめるなら、実際にプラグインを作って、`process` に来る値をログに出す。
- **この PC の Surge XT（VST3）が使っている JUCE の版**と、その版のラッパーのソース。JUCE 6.1.6、8.0.12、最新で同じなので、同じと見てよいが、Surge XT の実物（`Surge XT.vst3`）で、1 ブロックに 3 点送って、実際に最後の値だけ効くか、測るのが確かめになる。
- Surge XT の `BLOCK_SIZE`（32 と記憶、未確認）と、VST3 の道でパラメータが内部ブロックの頭にしか入らないかの確認（`SurgeSynthProcessor.cpp` の全体と `surge-xt/util` を読むとよい）。
- LSP、ZamComp、Dragonfly など、JUCE でない VST3 のプラグインが、1 ブロックの複数の点をどう読むか（点の位置を使うか）。DPF と LSP のソースを読む。
- **meloet のエンジンが、ループの折り返しをブロックの途中でどう扱っているか**（`crates/meloet-engine/src/engine.rs`）。分けているなら、ProcessContext の位置はブロックごとに正しい。
- Steinberg の `IProcessContextRequirements` の説明を、ヘッダ全文で読むこと（今回は要約のみ）。`ivstaudioprocessor.h` の `ProcessData` と `IAudioProcessor::process` の注意書き（「`numSamples == 0` のとき」など）も、全文では読んでいない。
- `IEditControllerHostEditing` の使い道（ホストが窓のつまみを動かす間の「始まりと終わり」）の、ヘッダの説明（B の投稿でしか読んでいない）。
