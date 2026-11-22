# 音の入口（cpal 0.18.2 の PipeWire）と、録った音の位置

調べた日 2026-11-21（meloet-research）。確かさは A・B・C。[S番号] は最後の表。PipeWire は、この PC に入っている 1.6.2。cpal は 0.18.2、pipewire クレートは 0.10.1（手もとのソース）。Ardour は GitHub の master を今日読んだ（特定の版ではない）。

## 結論（先に）

1. **入口の流れは、出口とは別の物として立つ**（A [S1][S2]）。`build_input_stream_raw` が 1 回ごとに、専用のスレッド `pw_in`、専用の `MainLoop` ・ `Context` ・ PipeWire への接続、`pw_stream` を 1 本つくる。出口（`pw_out`）とスレッドも接続も共有しない。起こしたあと `stream.play()` が要る（`INACTIVE` で立つ）。meloet の出口の作りと同じ形で、`DeviceId::new(HostId::PipeWire, node_name)` で探して `build_input_stream` すればよい。
2. **入口の `capture` と出口の `playback` は、同じ時計から作られる**（A [S1][S5][S6]）。どちらも `pw_stream_get_time_n` の `now`（グラフの driver が今のコマ（cycle）を始めた時刻。`CLOCK_MONOTONIC`）が元で、`capture = now − delay`、`playback = now + delay`。meloet の `monotonic_us()`（`CLOCK_MONOTONIC`）や ALSA シーケンサで読む鍵の時刻とも同じ時計（A [S14]）。
3. **入口と出口の間のドリフトは、cpal の外（PipeWire の中）で消える**（A [S8][S11]、一部推測）。cpal は入口と出口に同じ `node.group = cpal-<pid>` をつけ、「同じグループは必ず同じ driver で動く」ので、同じコマ・同じ `now` で動く。別々の装置なら、driver でない側の装置は PipeWire がリサンプラで合わせる（`clock.name` が同じ装置はリサンプラなし、と文書にある）。cpal の側からは、1 本のタイムラインに見える。
4. **cpal から取れる遅れは 2 つだけ**: 入口 `callback − capture`（= `delay`）と、出口 `playback − callback`（= `delay`）。`queued` ・ `buffered` ・ `size` ・ `ticks`、ノードの `Latency` の中身は cpal に出てこない。meloet が `pipewire` クレートで直接読む回り道はあるが、読むだけにする（決まりのとおり）。
5. **PipeWire の `delay` に入らない物**: DAC ・ ADC ・ コーデック ・ USB 機器の中の遅れ（出口の分は meloet の調べで確認済み。入口の分は同じ作りなので同じと推測、C）。**ほかの DAW は、ここを「手で入れる数」か「ループバックで測った数」で足している**（Ardour: 入口の遅れ ＋ 出口の遅れ。REAPER: ドライバーの数 ＋ 手の数。Audacity: 1 つの ms の数）。
6. **meloet の式に入る数**（私の導き、入力は A。手順は 5 章。確かさは C 寄りの推論）:
   - 音（歌）: `位置 = heard_at( capture_us − X )`。`X = D_in_ext + D_out_ext`（cpal が知らない外の遅れの合計）。
   - 鍵（MIDI）: `位置 = heard_at( t_key − D_out_ext )`。
   - **`X` は、出口から入口へ線をつないだ 1 回の測定で、`X = c − p` と出る**（`p` = 出した音の cpal の `playback` ＋ こま位置、`c` = 返ってきた音の cpal の `capture` ＋ こま位置）。cpal の時刻が元なので、Ardour のような「エンジンの遅れを引く」手間はいらない。**`D_in_ext` と `D_out_ext` を分けることは、音のループバックではできない**（合計だけ出る。jack_delay は半分ずつにする）。鍵だけを置くなら出口の分 `D_out_ext` だけが要る。
7. **cpal で取れない、または確かめられていない数と、回り道**:
   - `capture` が「ブロックの最初のこま」の時刻か「最後のこま」かは、cpal も PipeWire も文書で決めていない（差は 1 ブロック、256 こま 48 kHz で 5.3 ms）。→ 回り道: **ソフトだけのループバック**（出口のモニター `STREAM_CAPTURE_SINK`、または null シンク）で `X_sw = c − p` を測る。外の遅れが 0 なので `X_sw ≈ 0` のはず。ずれたらその量が cpal の約束のずれ。quantum 64〜1024 で回して、quantum に比例するかを見る（meloet が持つ自分用 PipeWire の試験の枠で回せる）。
   - `D_in_ext` ・ `D_out_ext` そのもの: 回り道は手で入れる数（ほかの DAW と同じ）と、ループバックの 1 回の測定。MIDI の鍵盤 → ALSA の遅れは音のループバックでは測れない（Ardour は MIDI 専用の測定 `MIDIDM` を別に持つ [S17]）。
8. **向きの注意（architecture.md 12 章の最後の項への食いちがい、私の推論 C）**: 12 章は「入っていない遅れの分だけ、録った位置は早めに出る」と書く。私の手順では、出口の外の遅れ `D_out_ext` を知らないまま `heard_at(t_key)` を使うと、録った鍵は**遅めに**出る（人は、エンジンが言うより `D_out_ext` 遅れて拍を聞くので、その分遅く押す）。向きの確認を。

## 1. cpal 0.18.2 の pipewire で、入口をひらく作法

### 1-1. デバイスのえらびかた（A [S2][S3]）
- `host.devices()` は、PipeWire のノードを集めた表（ホストを作ったときの 1 回のスナップショット）。ノードの `media.class` が `Audio/Sink` ・ `Audio/Source` ・ `Audio/Duplex`（と `/Internal`）・ `Stream/Output/Audio` ・ `Stream/Input/Audio` のものが入る。
- `Audio/Source` は `DeviceDirection::Input`。**`Audio/Sink` は `Duplex` として入る**ので、`supports_input()` が真になる。入口を sink に向けると、`stream.capture.sink = true` がつき、**その sink のモニター（出口に流れる音）を録る**（ソフトのループバックに使える）。`Audio/Duplex` も `Duplex`。画面の「入口の一覧」は、`direction() == Input` で絞るのが、sink を混ぜない近道（`description().direction()` で見える）。
- 名前で探すとき: `d.supports_input() && d.id() == DeviceId::new(HostId::PipeWire, node_name)`。見つかった `Device` は `Class::Node` で、`target.object = <object.serial>` がつく（その装置につなぐ）。`DONT_RECONNECT` もつく（装置が消えると `DeviceNotAvailable` が error callback に来る）。
- `host.default_input_device()` は `Class::DefaultInput`。`target.object` なしで立ち、WirePlumber が `default.audio.source` につなぐ。**デフォルトが変わると error callback に `DeviceChanged` が来る**（A [S1]）。
- 入口をひらく前に `Host` を作りなおす（meloet の出口と同じ）。装置の表がホストを作った時点のものだから。

### 1-2. チャンネル数 ・ サンプルの形 ・ 率（A [S2][S1]）
- `default_input_config()`: 形は **F32**、チャンネル数は**ノードの `audio.channels`**（ないときは 2）、率は `node.rate`（なければ `clock.rate`）、バッファは `clock.min-quantum..clock.max-quantum`。
- `supported_input_configs()`: チャンネル数は装置の 1 通りだけ。率は `clock.allowed-rates`（なければ 1 つ）。形は I8 U8 I16 U16 I24 U24 I32 U32 F32 F64（**I64 ・ U64 は libspa に対応がないので入らない**）。
- 立てるとき、cpal は `EnumFormat` に「形 ・ 率 ・ チャンネル数」を**固定で**出す。`NO_CONVERT` は立てない（A [S5]）ので、PipeWire の stream 側の変換が、装置との差（チャンネル数など）を埋める（推測、C）。それでも、**出口のように 2 を決め打ちにせず、`default_input_config()` のチャンネル数と率を使う**のが安全（マイクは 1 チャンネルが多い）。
- もし結ばれた形が頼んだ形とちがうと、`param_changed` が `UnsupportedConfig`（"Negotiated format mismatch …"）を error callback に出し、`set_active(false)` で止める（A [S1]）。

### 1-3. バッファの大きさの頼みかた（A [S2][S1][S11]）
- `StreamConfig.buffer_size = BufferSize::Fixed(n)` → ストリームの属性 **`node.latency = "n/率"`**。PipeWire の文書では**ただの希望**（「グラフはこの遅れか、それより小さくしようとする」）。meloet の出口と同じく、実際の大きさは `Stream::buffer_size()`（`last_quantum`、毎回のコマの大きさ）で見る。
- `Fixed(n)` が `min_quantum..=max_quantum` の外なら `UnsupportedConfig` で断られる（`max_quantum == 0`、つまり設定がまだ届いていないときは通る）。`Default` なら何も頼まない。
- **quantum はグラフ全体で 1 つ**（PC の設定。meloet は変えない）。入口と出口を同じプロセスでひらいても、ふつうは同じ大きさで回る。ただし**入口の 1 回の呼びだしの長さは、毎回同じとは限らない**: 入口の callback は `chunk.size / (サンプルの大きさ × チャンネル数)` を `frames` にして渡す。`pw_time.size` の文書は「capture の stream では、今のコマで取れるサンプル数」と言う（A [S5]）。装置がリサンプラでつながっているときは、quantum の前後でゆれる（推測、C）。0 こまの呼びだしもありうる。入口 → エンジンの輪は、長さが変わる書きこみを受ける形にする。

### 1-4. 流れとスレッドの作り（A [S1][S2][S5][S12]）
| 項目 | 内容 |
|---|---|
| ストリーム | 入口 1 本につき `pw_stream` 1 本。名前 `cpal-capture-<pid>`（出口は `cpal-playback-<pid>`） |
| スレッド | 入口 `pw_in`、出口 `pw_out`。それぞれ自分の `MainLoopRc` ・ `ContextRc` ・ 接続（`remote_props()` で `pipewire-0` のソケットを探す）。`PwInitGuard` で `pw_init` を数える |
| 共通の属性 | `media.type=Audio`、入口は `media.category=Capture`（出口は `Playback`）、`node.group=cpal-<pid>` |
| フラグ | `MAP_BUFFERS | INACTIVE`（＋`AUTOCONNECT`、非デフォルトの装置は `DONT_RECONNECT`）。**`RT_PROCESS` は立てない**。cpal のコメント: process は、この main loop のスレッドで動く |
| 非 RT の意味 | PipeWire 1.6.2 の `pw_stream_connect` は、`RT_PROCESS` がないと **`node.loop.class=main` と `node.async=true`** を足す（A [S12]。要約ツールを通して読んだ。全文は読んでいない）。「非同期のリンクは 1 コマの遅れを足す」（A [S9][S10]）。この 1 コマが `pw_time.delay` に入っているかは、**確かめていない**（8 章） |
| 実時間化 | cpal の `realtime` 機能は、meloet の `Cargo.toml` で入れていない（`features = ["pipewire"]`、`default-features = false`）。cpal は優先度を上げない。meloet の `plughost/rt.rs` のコメントは「PipeWire が meloet の音のスレッドに 88 を与える」と言う。ここは調べていない |
| 起動 | `play()` が `set_active(true)`。止めるときの `Drop` は `disconnect` ＋ `quit` ＋ join |
| 順序 | 入口と出口の callback は、**別のスレッドで、順序の約束なし**。同じコマの入口の音を、同じコマの出口に使うことはできない（輪を通すと最低 1 ブロック遅れる）。時刻（`now`）は同じコマなら同じ値（A [S1][S5]） |

## 2. 入口と出口の時刻（A）

### 2-1. cpal の `StreamInstant` は何から作られるか（A [S1][S3][S4]）
`pw_stream_time()` は `pw_stream_get_time_n`（pipewire クレートは `v0_3_53` 機能で `_n` 版を使う。cpal の `Cargo.toml`）の結果。`now <= 0` か `rate.denom == 0`（driver がまだ始まっていない）なら `None`。

| | 入口 `InputStreamTimestamp` | 出口 `OutputStreamTimestamp` |
|---|---|---|
| `callback` | `now`（ns） | `now`（ns） |
| もう片方 | `capture = now − delay_ns` | `playback = now + delay_ns` |
| `delay_ns` | `max(delay, 0) × 1e9 / rate.denom`（負の遅れは 0 に切る） | 同じ |
| `None` のとき | `callback = clock_gettime(CLOCK_MONOTONIC)`、`capture = callback − frames/率` | `callback = clock_gettime`、`playback = callback + frames/率` |

- **`callback` は「callback が動いた時刻」ではなく「グラフがこのコマを始めた時刻」**（A [S6]: `spa_io_clock.nsec` は「現在のコマが始まった実時刻。ジッターがありうる」）。
- `capture` は呼びだしをまたいで**後ろへ戻らない**ように切る（`monotonic_input_callback`。別のクライアントが入って `delay` が増えたときの対策。A [S3]）。出口の `playback` も同じ。
- 立ち上がりの数コマは、`None` の道（callback の時刻をもとにした見積り）を通りうる。cpal はどちらの道を通ったかの印を出さない。`callback − capture` が `frames/率` にちょうど等しいときは見積りの道、が見分けの目安（推測、C）。
- 時計の名前: cpal のコメントは「`monotonic_stream_instant()` は `pw_stream_get_time_n` と同じ時計」と書く（A [S1]）。meloet の `monotonic_us()` は `CLOCK_MONOTONIC`（A [S14]）。**そのまま足し引きできる**。

### 2-2. PipeWire の `pw_time` の意味（A [S5][S12]）
- `now`: 時間報告を更新した時刻（ns）。ふつう 1 コマごと。`ticks`: そのときの driver の位置。`rate`: `ticks` と `delay` の単位（`1/サンプル率`）。
- **`delay`**: 「装置までの遅れ。出口は、stream の次の出力サンプルが再生装置で出るまでの時間。入口は、サンプルが入力装置から来るまでにかかった時間。途中のフィルターと、足された遅れの手直しを含む。グラフの形か quantum か率が変わらなければ一定。**queued なバッファによる遅れは含まない**。負にもなりうる」。
- `delay` の中身は、stream が持つ `Latency` パラメータの中間値 `(min_quantum+max_quantum)/2 × quantum ＋ (min_rate+max_rate)/2 ＋ (min_ns+max_ns)/2 を率に直した数`（`pw_stream_get_time_n`。A [S12]）。この `Latency` に、グラフの途中の遅れ（フィルター、非同期のリンクの 1 コマなど）と、装置のノードの `ProcessLatency` が積まれる（A [S10]）。
- 出口の合計の遅れの式（A [S5]、文書のまま）: `buffered / 率 ＋ queued / app率 ＋ (delay − elapsed) × rate.num / rate.denom`。**cpal は `queued` と `buffered` を足さない**。`buffered` は変換器（リサンプラ）の中のこま数。リサンプラを通さない形（F32、装置と同じ率）なら 0 に近いはず（推測、C）。
- ALSA の装置（`alsa-pcm.c`、A [S13]。要約ツールを通して読んだ。全文は読んでいない）: `clock->delay = delay + state->delay`。`delay` は ALSA のバッファにたまっているこま数（出口はバッファ全体 − 空き、入口は取れるこま数）。`state->delay` は rate match（リサンプラ）があるときの `rate_match->delay`。ノードが報告する遅れは `headroom`（`min_delay..max_delay` に切る）。`latency.internal.rate` / `latency.internal.ns` という設定があり、ノードの `process_latency` に入る。**ALSA の `snd_pcm_delay` は見つからなかった**。DAC ・ コーデック ・ USB の箱の中の遅れが入らないのは、このため（出口は meloet の前の調べ、PipeWire 1.4 のソースでも同じ結論。入口の分は 1.6.2 で読んだ範囲では同じ作り）。
- 「手で足す遅れ」の仕組み: `pw_time.delay` の文書に「ユーザーが同期のために遅れの手直し（offset）を足すことがある」とある（A [S5]）。**どの設定名で、どの道具で入れるかは確かめていない**（ProcessLatency を手で入れたという Ardour のフォーラムの話があるが、C）。

### 2-3. ドリフト（A [S8][S11][S6]、推測を分けて書く）
- 同じプロセスの入口と出口は、`node.group` が同じ → 「同じ driver」で動く（A [S8]）。driver が決まれば、入口も出口も同じコマの `clock.nsec` を使う。
- 入口と出口の装置が**同じカードなら同じ時計**。別のカードなら、driver でないほうの装置は rate match（リサンプラ）で driver に合わせる（文書: 「`clock.name` が同じ装置は、時計を合わせるリサンプラを使わない」A [S11]。裏返して「ちがえば使う」は推測、C）。このとき装置ノードの `delay` に、リサンプラの遅れ `rate_match->delay` が入る（A [S13]）。
- 手もとの測定（B [S16]、meloet 自身の記録）: 出口の装置の時計と `CLOCK_MONOTONIC` の差は **+6.5〜6.9 ppm**（10 分で約 +3.9 ms）。10 分の録音では、**ブロックごとに時刻から位置を出すと、フレームを数えて進む曲の位置とずれていく**。録音の頭の位置だけを時刻から出し（アンカー 1 個）、あとはこま数で数えて並べるほうが、曲の進みと同じ時計になる（同じコマの driver でつながっているため。導き: 入力は A と B、確かさ C）。入口の装置が別でも、PipeWire が driver のこま数にそろえて渡す。

### 2-4. PipeWire が言う入口と出口の遅れの、数の取りかた
| 道 | 取れる物 | cpal から | 確かさ |
|---|---|---|---|
| `pw_stream_get_time_n`（RT で呼べる） | `now ・ ticks ・ rate ・ delay ・ queued ・ buffered ・ size` など | `delay` だけが `capture` ・ `playback` の形で見える | A [S5][S12] |
| stream 自身の `SPA_PARAM_Latency` | 下流 / 上流の遅れの範囲（`min/max` の quantum ・ rate ・ ns の 3 つ組） | 見えない | A [S10][S7] |
| ノードの `ProcessLatency`（`SPA_PARAM_ProcessLatency`） | そのノードの処理が足す遅れ（quantum ・ rate ・ ns） | 見えない | A [S10][S7] |
| ノードの属性 `node.latency`、`node.rate` | **「頼んだ」quantum と率**（測った遅れではない） | 列挙のときだけ内部で使う（出口の `default_*_config` の中身に入る） | A [S2][S11] |
| `pipewire` クレート 0.10.1 の `Node::enum_params(Latency)`（meloet-devices が `pipewire` を持つ） | 装置ノードの `Latency` と `ProcessLatency` | meloet が直接読む。書く道（`set_param`）は meloet では使わない | A（API の存在のみ。使ったことは確かめていない） |

- 向きの約束（A [S10]）: 出力ポートの `Latency` は「上流の合計」で JACK の capture latency に当たり、入力ポートの `Latency` は「下流の合計」で JACK の playback latency に当たる。リンクが複数あるときは min の min と max の max の範囲になる。
- `ProcessLatency` や `pw_time.delay` の「手直し」を、PipeWire の側へ書く道は、PC 全体の設定を変えない決まりに合わせて**使わない**。meloet の自分の設定（曲の中でなく、アプリの設定）で `X` を持ち、`heard_at` の引数に入れるだけにする（推奨、C）。

## 3. ほかの DAW は、録った音の位置に何を足し引きしているか

| DAW | 足し引きする物 | 手で入れる数 | 測りかた | 確かさ |
|---|---|---|---|---|
| **Ardour** | **音**: 録ったあと、窓 `_first_recordable_sample` に `_capture_offset + _playback_offset` を足す（`DiskWriter::check_record_status`、`ExistingMaterial` のとき。ループ録音は `_capture_captured -= _playback_offset + _capture_offset`）。`_capture_offset` = ルートの入口ポートの capture 遅れ `_input->latency()` ＋ プロセッサの積み上げ。`_playback_offset` = `_signal_latency + _output_latency`（出口ポートの遅れ）。**MIDI**: イベントの時刻から `_accumulated_capture_offset` を引く。その初めの値は `_playback_offset`（ループは位置の差も足す）。入口の物理ポートでなければ、`CaptureTime` にして足さない | 設定の「Hardware input latency」「output latency」（`set_systemic_input_latency` / `output`）がポートの遅れに足される | 「Calibrate Audio」（下の 4 章） | A [S17][S18]（GitHub の master。要約ツールを通して読んだ。`_capture_offset` が MIDI で 0 になる件は livetrax の commit 名 "Zero capture-offset regardless of data-type" を見ただけで、本文は読めていない = B） |
| **REAPER** | ドライバーが報告する遅れで自動で合わせ、さらに手の数を足せる。**入口から来た信号にだけ**効く（トラックからトラックへ回した音には要らない） | 録音の遅れの手の数（サンプルか ms。負も可。自動と足し算） | ReaInsert の「ping detect」で、出口 → 入口の線をつないで遅れ（サンプル）を測り、手の数に足す | C [S23]（`reaper.blog`、書き手は "Admin" でコッコスの人かは確かめられない。Cockos の wiki は取れず、検索の抜粋だけ） |
| **Audacity** | 1 つの「Latency compensation」の数（ms）。マニュアルは「ふつうの機器でおよそ 130 ms」「機器と設定で変わるので、マルチトラックでは測って合わせる」 | その 1 つの数 | 「Latency Test」のページ（読めなかった） | A [S24]（マニュアル。数の既定値は読めず、範囲の話のみ） |
| **Bitwig** | **公式の文書は見つからなかった** | 「Recording Offset」。上限が 2048 こま（約 43 ms）と、2026 年 6 月の KVR の投稿に（Bitwig 6.1 の利用者の話） | 見つからなかった | C [S25] |

- Ardour の遅れは、JACK 流の「ポートごとの遅れの範囲」から来る。PipeWire は同じ形の `Latency` を持つ（2 章）。Ardour の公式マニュアルは、JACK の `-I` / `-O`（装置の外の遅れを JACK に教えるつまみ）を「システムごとに違うが、ずっと一定」と説明する（A [S21]）。
- Paul Davis（Ardour の作者）の昔の発言: コンバーターの遅れはドライバーに分からないので、利用者が測ってつまみに入れる（C [S22]。古いので 1 段下げて C）。
- **どの DAW も、「ソフトが知っている遅れ」と「手か測定で足す外の遅れ」の 2 本立て**。meloet の `X`（`D_in_ext + D_out_ext`）は、後者に当たる。

## 4. 出口から入口へ音をまわして遅れを測る作法

読むだけ。**コードは写さない**（Ardour の `mtdm.cc` と jack2 の `tests/iodelay.cpp` は、どちらも GPL-2 以降。「Fons Adriaensen 2003-2012 ほか」。A [S19][S20]。meloet の MIT / Apache-2.0 には入れられない）。

### 4-1. Ardour の「Calibrate Audio」と jack_delay（同じ MTDM の方法）
- **音（A [S19][S20]）**: 13 本のサイン波の和を出口に出す。基準の音（3 kHz = 率/16、周期 16 こま、振幅 0.20）と、ほかの 12 本（0.01 ずつ。周期 17.1〜60.2 こま、48 kHz で約 800〜2810 Hz）。ピークは約 0.32。周波数は 16 ビットの位相の足し算（`f / 65536` サイクル/こま）で決める。
- **見つけかた**: 返ってきた音を、13 本それぞれの sin ・ cos と相関させ、16 こまごとの和を、2 段の 1 次のローパス（係数 `200/率`、1 段の時定数 約 80 ms。計算の入力は A）に通す。基準の音の位相で「粗い遅れ」を出し、残りの 12 本の位相で、1 本ずつ 1 ビットずつ整数のサイクル数の曖昧さを消す（バーニアの形）。結果 `del = 16 × d`（こま）。
- **確かさの印**: `err` = 12 本のうち、いちばん合わなかった物の、半サイクル整数からのずれ（0〜0.5）。`resolve()` は、基準の音の大きさが 0.001 未満なら −1（"No signal"）、`err > 0.4` なら 1（信頼できない）。極性が逆（`err > 0.3` で入れかえてもう 1 回）にも対応。Ardour の窓は、`err ≤ 0.2` を「solid」としてはじめて「使う」ボタンを押せるようにし、ピークが 0.707（−3 dBFS）を超えたら入力が大きすぎると言う（A [S18]）。
- **平均の回数**: 「何回の平均」という数は**ない**。ローパスの連続の平均で、100 ms ごとに読み、最初の solid な読みを採る。jack_delay は 250 ms ごとに表示し、`err > 0.2` に "??" をつける（A [S18][S20]）。
- **確かさの量**: JACK のマニュアルは「**サブサンプルの精度**」と言う（A [S21]）。ここでの数値は文書にない。
- **引き算**: Ardour は `extra = mtdm->del() − latency_signal_delay`（エンジンが報告する入口 + 出口の遅れ。コマの数の世界）。jack_delay は `floor(del − (capture 最大 + playback 最大))` を「systemic latency」とし、**半分ずつ**を `-I` ・ `-O` の案として出す（A [S18][S20]）。測り始めの前に 1 周期ぶんをからにする（`_latency_flush_samples`。A [S17]）。
- **つなぎかた**: 出口 → 入口を線でつなぐ（スピーカー → マイクもありうるが、まれ）。デジタルの輪では AD / DA の遅れは入らない（A [S21]）。
- **USB の注意**: 抜き差し、再起動、xrun のあとで USB 機器の遅れが変わることがある。**セッションの頭ごとに測りなおす**よう勧める（A [S21]）。
- **MIDI**: 別の `MIDIDM` で、MIDI の出口 → 入口の線で測る（A [S17][S18]。方法の細かい所は読んでいない）。
- **Ardour の対応する裏付け**: Ardour の「Calibrate」は ALSA バックエンドの話で、JACK（PipeWire-JACK を含む）は jack_delay を使うよう案内される（A [S21]、検索の要約では C。確かさは A の範囲だけ採る）。

### 4-2. ほかの作法
- REAPER: ReaInsert の「ping detect」（出口 → 入口の線をつないで遅れをこまの数で出し、手の数に足して、もう一度押して「0 samples」になることを確かめる）。**ping の形（音の種類、しきい値）は見つからなかった**（C [S23]）。
- Audacity: 「Latency Test」の手順は読めなかった。一般に、出した音と録った音の波形を見て、手で数を決める作法と推測（C）。

### 4-3. meloet が自分で作るなら（提案、C。作法の考えだけを使い、コードは写さない）
- 音: 短い窓つきのクリック列か、広い帯のチャープ。1 回の長さは quantum より十分長く、間は遅れの最大（数十 ms 〜 100 ms）より長く空ける。
- 見つけかた: 出した音の波形（既知）との相互相関を取り、ピークを放物線で補間してサブサンプルにする。ピークと次に高い山の比（鋭さ）を確かさの印にする（MTDM の `err` に当たる物）。
- 回数: 16〜32 回出して、中央値と散らばり（MAD）を見る。散らばりが 1 こまをこえたら「やりなおし」。
- 引き算は **いらない**: `X = c − p`（`c`、`p` は cpal の時刻 ＋ ブロック内のこま位置）。
- 毎回測らなくてよいように、`(入口のノード名, 出口のノード名, 率, quantum)` を鍵にして、アプリの設定にしまう。入口か出口の装置、率、quantum が変わったら「測りなおして」と出す（USB の注意 A [S21] より）。
- ライセンスは、自分で書くので MIT / Apache-2.0 のまま。

## 5. 位置の式の導きかた（私の導き。入力は A、確かさは C 寄りの推論）

記号: `P(t)` = `heard_at(t)`（エンジンの言う「DAC に届く」時刻 `t` の曲の位置）。`D_out_ext` = DAC から人の耳までの、PipeWire が知らない遅れ。`D_in_ext` = マイクの音が ADC を通って、cpal の `capture` の時刻に着くまでの、PipeWire が知らない遅れ。

1. 拍の位置 `P0` の音が、エンジンの言う `T0` に DAC へ届くと、人の耳には `T0 + D_out_ext` に聞こえる。
2. 人は聞こえた拍に合わせて歌う。音がマイクに着く（空気の時間は無視）のは `a = T0 + D_out_ext`（＋人の反応）。
3. その音は ADC を通り、cpal の `capture` の時刻 `c = a + D_in_ext` のこまとして現れる。
4. だから、`c` のこまを置く曲の位置は、`T0 = c − D_in_ext − D_out_ext` のときの `P0`、すなわち **`P(c − X)`**（`X = D_in_ext + D_out_ext`）。
5. 鍵は、鍵盤 → ALSA の遅れ `D_key` を別に持つ（今は 0 と置く）。押した時刻 `t_key` の位置は `P(t_key − D_key − D_out_ext)`。
6. ループバック: 出したこまの cpal の `playback` を `p`、返ってきたこまの cpal の `capture` を `c` とすると、ソフトの外の遅れだけが残り、`c − p = D_out_ext + D_in_ext = X`。ソフトだけの線（sink のモニター）なら `X_sw = c − p` が 0 のはず。これが 0 からずれるなら、cpal の `capture` ・ `playback` の約束（最初のこまか最後のこまか、非同期の 1 コマが `delay` に入るかなど）が、約束どおりでない分。

## 6. meloet への具体の提案（C。決めるのは棟梁）

- 入口をひらく係は `crates/meloet-engine/src/output.rs` と同じ形で、`input.rs` を足す。callback は `info.timestamp()` の `capture`（ns）と、受けたこま数を、輪（長さ可変）に入れるだけ。鍵と同じ `monotonic_us()` の世界の `capture_us` を持ち越す。
- 録音の位置は、`heard_at(capture_us − X)` を**頭だけ**に使い、あとはこま数で数える（2-3 章）。
- 試験の順: (1) ソフトのループバックで `X_sw` を、quantum 64〜1024 で測る。(2) 実機のループバック（出口 → 入口の線）で `X` を測る。(3) 安定を、抜き差しと再起動で見る。
- `docs/architecture.md` 12 章の最後の項は、向き（8 章）と「入口の外の遅れも入らない」を書き足す案。

## 7. 出どころ

| 番号 | 出どころ | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/cpal-0.18.2/src/host/pipewire/stream.rs` | `connect_input` ・ `connect_output`、`publish_data_in` ・ `publish_data_out`、`pw_stream_time`、`monotonic_stream_instant`、フラグ、スレッド、`param_changed` の形の検査、`check_xrun` | A | 2026-11-21 |
| S2 | 同 `device.rs` | `build_input_stream_raw`、`pw_properties`（`node.group`、`node.latency`、`stream.capture.sink`、`target.object`）、`default_input_config`、`init_devices`（Sink は Duplex、ノードの `node.latency` ・ `node.rate`） | A | 2026-11-21 |
| S3 | 同 `src/host/mod.rs`（`monotonic_input_callback` ほか）、`pipewire/mod.rs`、`utils.rs`、`Cargo.toml` | 後ろへ戻さない切り、`default_input_device`、`realtime` 機能、`pipewire` の `v0_3_53` 機能 | A | 2026-11-21 |
| S4 | 同 `src/timestamp.rs` | `InputStreamTimestamp` ・ `OutputStreamTimestamp` と、各ホストの時計の表（PipeWire は `pw_stream_get_time_n()`） | A | 2026-11-21 |
| S5 | `/usr/include/pipewire-0.3/pipewire/stream.h`（PipeWire 1.6.2、`version.h` で確認） | `struct pw_time` と各欄の意味、`delay` ・ `queued` ・ `buffered` ・ `size` の文書、遅れの式、`RT_PROCESS` ・ `ASYNC` のフラグ | A | 2026-11-21 |
| S6 | `/usr/include/spa-0.2/spa/node/io.h` | `spa_io_clock`（`nsec` は `CLOCK_MONOTONIC`、コマの始まり、`rate_diff`、`flags`） | A | 2026-11-21 |
| S7 | `/usr/include/spa-0.2/spa/param/latency-utils.h` | `spa_latency_info` と `spa_process_latency_info` の積み方 | A | 2026-11-21 |
| S8 | https://docs.pipewire.org/group__pw__keys.html | `node.group`（同じグループは同じ driver）、`node.latency`、`node.link-group` | A | 2026-11-21 |
| S9 | https://docs.pipewire.org/page_scheduling.html | driver と follower、非同期のリンクは 1 コマの遅れを足す | A | 2026-11-21 |
| S10 | https://docs.pipewire.org/page_latency.html | `Latency` の向き、合成、`ProcessLatency` | A | 2026-11-21 |
| S11 | https://docs.pipewire.org/page_man_pipewire-props_7.html | `node.latency` は希望、`clock.name` が同じならリサンプラなし | A | 2026-11-21 |
| S12 | https://raw.githubusercontent.com/PipeWire/pipewire/1.6.2/src/pipewire/stream.c（PipeWire 組織の GitHub ミラー） | `pw_stream_get_time_n` の中身、`copy_position`、`RT_PROCESS` なしで `node.async=true` と `node.loop.class=main` を足す。**要約ツールを通して読んだ。全文は読んでいない** | A（一部のみ確認） | 2026-11-21 |
| S13 | https://raw.githubusercontent.com/PipeWire/pipewire/1.6.2/spa/plugins/alsa/alsa-pcm.c | `clock->delay = delay + state->delay`、headroom、`latency.internal.*`。ファイルの後ろ 25 KB は読んでいない。**要約ツールを通して読んだ** | A（一部のみ確認） | 2026-11-21 |
| S14 | `/srv/meloet-agent/meloet/crates/meloet-engine/src/output.rs` ・ `clock.rs` ・ `engine.rs`（`monotonic_us`）、`docs/architecture.md` 11 ・ 12 章 | meloet の出口の作り、`heard_at`、`CLOCK_MONOTONIC`、入っていない遅れの項 | A（プロジェクト自身） | 2026-11-21 |
| S15 | `/srv/meloet-agent/meloet/Cargo.toml`（cpal の機能）、`crates/meloet-plughost/src/rt.rs` のコメント | `realtime` 機能なし。PipeWire が 88 を与えるという書きぶり | A（書いてあることのみ。事実の確認はしていない） | 2026-11-21 |
| S16 | `/srv/meloet-agent/meloet/docs/measurements/2026-11-20-round6-step1.md` ほか | 出口の時計と `CLOCK_MONOTONIC` の差 +6.5〜6.9 ppm、録った位置の試験は押す時刻もエンジンも同じ値で見えない範囲があるという記録 | B（自分の測定。再確認はしていない） | 2026-11-21 |
| S17 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/disk_writer.cc ・ `route.cc` ・ `io.cc` ・ `audioengine.cc`（`master`） | `_capture_offset + _playback_offset`、`_input->latency()`、`_signal_latency + _output_latency`、MIDI のイベント時刻、MTDM / MIDIDM の入口と出口のポート。**要約ツールを通して読んだ。ファイルの一部のみ** | A（一部のみ確認） | 2026-11-21 |
| S18 | https://raw.githubusercontent.com/Ardour/ardour/master/gtk2_ardour/engine_dialog.cc | `extra = mtdm->del() − latency_signal_delay`、`err` の 0.2 / 0.3 の扱い、100 ms ごとの確認、ピーク 0.707。**要約ツールを通して読んだ** | A（一部のみ確認） | 2026-11-21 |
| S19 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/mtdm.cc ・ https://ardour.org/files/doxygen/mtdm_8h_source.html | 13 本の音の表、振幅、位相の計算、`resolve` の手順、しきい値。GPL-2 以降 | A | 2026-11-21 |
| S20 | https://raw.githubusercontent.com/jackaudio/jack2/master/tests/iodelay.cpp | jack_delay の方法、systemic latency の引き算、半分ずつの案。GPL-2 以降 | A | 2026-11-21 |
| S21 | https://manual.ardour.org/synchronization/latency-and-latency-compensation/ | capture / playback / round-trip の定義、`-I` / `-O`、jack_delay、サブサンプルの精度、USB の注意 | A | 2026-11-21 |
| S22 | https://discourse.ardour.org/t/latency-compensation-how-is-output-latency-accounted-for-in-ardour/82919 | Paul Davis の発言（コンバーターの遅れは利用者が測る）。古い投稿 | C（開発者本人の発言で B だが、2 年超で 1 段下げ） | 2026-11-21 |
| S23 | https://reaper.blog/2018/11/rec_latency_offset/ ・ 検索の抜粋（Cockos wiki のページは取れず） | REAPER のドライバーの数 ＋ 手の数、入口にだけ効く、ReaInsert の ping detect | C（書き手が確かめられない。2018 年） | 2026-11-21 |
| S24 | https://manual.audacityteam.org/man/audio_settings_preferences.html | 録音の遅れ（約 130 ms）、補正の意味、測って合わせる | A | 2026-11-21 |
| S25 | 検索結果の KVR の投稿（Bitwig 6.1 の Recording Offset の上限 2048 こま） | Bitwig の手の数 | C（Bitwig の公式文書は見つからなかった） | 2026-11-21 |

## 8. わからなかったこと、追加で調べるとよいこと

- **一次情報が見つからなかった物**: Bitwig の録音の遅れの扱い（公式の文書なし）、REAPER の公式の文書（Cockos の wiki は取れず、ユーザーガイドの PDF は大きすぎて読めなかった）、Audacity の「Latency Test」の手順（ページが取れなかった）、ReaInsert の ping の形。
- **`capture` のこまの約束**（最初か最後か）と、**非同期（`node.async`）の 1 コマが `pw_time.delay` に入っているか**は、文書にもソースの読んだ範囲にもなく、確かめていない。ソフトのループバックで `X_sw` を測れば決まる（5 章 6、7 章 の提案）。私は実機で動かせない（読むだけ）。
- **PipeWire が `queued` を出口の位置に足すべきか**: cpal は足さない。`buffered` は変換のないとき 0 に近いと見るが、測っていない。
- **入口の `delay` の中身**（ALSA の入口で、`delay` が `snd_pcm_avail` の「たまっているこま数」であることと、ブロックの何番めのこまの時刻かの関係）は、`alsa-pcm.c` の読みこみの途中までで、確かめきれていない。
- **PipeWire の「手で足す遅れ」の設定名と道具**（ProcessLatency の値を、ユーザーが入れる道）は見つからなかった。meloet は PC の設定を変えない決まりなので、いまは使わない案にしている。
- **Ardour**: `_playback_offset` の効き方（音の置き直しの向き）を、窓の足し算の読みから推した。実際に録って確かめてはいない。MIDI の `capture_offset` が 0 になる件は、livetrax の commit 名しか見ていない（Gitea の画面が取れなかった）。
- **別の装置の入口と出口**で、PipeWire のリサンプラが `delay` にどれだけ足すかは、数を見ていない。
- **`pw_in` の callback が main loop で動く**（RT でない）ことの、時刻の一貫性（callback が遅れて、`now` が次のコマになる可能性）は、出口では 0.8 ms 以内で問題が出ていないが、入口で同じかは未確認。
- **MIDI の鍵盤 → ALSA の遅れ**: 音のループバックでは測れない。MIDI の出口 → 入口を線でつなぐ測り方（Ardour の `MIDIDM` と同じ種類）が要る。
