# プラグイン係（別のプロセス）との音のやりとりと見はり

調べた日 2026-11-19（meloet-research、Sonnet 5.5）。確かさは A（一次情報）・B（一次情報を引いた物）・C（推測・個人の投稿）。番号は最後の出どころの表。

## 結論

1. **ほかのホストは「同じブロックの中で往復」**。Carla と yabridge は、音のスレッドが子に音をわたして起こし、返事を待つ [A 1〜6]。「1 ブロックおくらせる」作りのホストは見つからなかった。Bitwig と REAPER の中の作りは公式の文書にない。
2. **Carla の見はりは meloet に合わない**。音のスレッドで最大 1000 ms 待ち、一度時間切れになると、そのプラグインは出力 0 のまま戻らない [A 4]。
3. **rtkit のはじめの値が、プロセスの数をしばる**（いちばん大事）。1 ユーザーでリアルタイムにできるのはプロセス 15・スレッド 25 まで、20 秒に 25 回まで、優先度は 20 まで、RLIMIT_RTTIME は 200 ms まで。rtkit でリアルタイムにしてもらうには `SCHED_RESET_ON_FORK` が要るので、**子は親のリアルタイムを受けつがない** [A 23, 20]。「80 個のプラグインを 80 プロセス」は rtkit の道では動かない（limits.d の rtprio がある PC は別）。Ubuntu 26.04 で値が変えられているかは未確認。
4. **起こしかたは futex（共有メモリの上、`_PRIVATE` なし）が第一候補**。Carla と JACK2 が使う [A 3, 35]。時間切れは相対で `CLOCK_MONOTONIC` [A 15]。futex は相手が死んだことを知らせないので、pidfd と時間切れで見はる [A 18]。
5. futex と eventfd を同じ条件でくらべた測定は見つからない。パイプ ・ eventfd の往復は中央値約 4.3 µs、先端は 30〜400 µs（2015 年の個人ブログ）[C 30]。**この PC で測る**。

## くわしく

### ほかのホスト
- **Carla**: プラグイン 1 つに子のプロセス 1 つ。音は共有メモリ（入力と出力のこまを 1 枚に）、音のスレッド用の小さな領域にセマフォ 2 つ・再生の位置・輪のバッファ・MIDI 出力。音でない物は別の輪 2 本。Linux では共有メモリの上の futex（0→1 で起こす）、なければ `sem_t`。待ちは同じブロックの中。時間切れ: 処理 1000 ms、activate 2000 ms、止める 3000 ms、状態の保存 60 秒、窓を埋めこむ 15 秒。子の生き死には 100 ms ごとにふつうのスレッドで見る。輪はアトミックなしで、セマフォの順序に頼る（読んだ推論 [B〜C]）[A 1〜5]。
- **yabridge**: 音は共有メモリ、ほかは Unix ソケット（ソケットが同期の役）。同じブロックで往復。Wine 側の音のスレッドの優先度を親に合わせて定期的にそろえる。RLIMIT_RTTIME が小さい、RLIMIT_MEMLOCK が足りないときは警告し、ロックなしで動く。子の出力は `disable_pipes` でファイルへ [A 6〜8, 34]。
- **Bitwig**: Within Bitwig ／ Together ／ By manufacturer ／ By plug-in ／ Individually の 5 つ。プラグインごとの上書きあり。中の作りと遅れの数は書いていない [A 9, 10]。
- **REAPER**: プラグインごとに Separate process（まとめる）／ Dedicated process（1 つずつ）／ Native only [B 11]。
- **Ardour**: 別のプロセスにしない。理由は 1 回の呼び出しに 2 回の切りかえ、1 回 30 µs と**仮定**して 100 トラックでスケールしない [A 13（数は仮定）]。
- **Zrythm**: はじめから別のプロセス。「DSP は少し遅くなるが、ふつうは十分速い」[A 14]。

### Linux の道具
- memfd はソケットで子にわたせる。`F_SEAL_SHRINK`・`F_SEAL_GROW` で相手に縮められない（縮められると `SIGBUS`）。`MAP_LOCKED` はゆるいロックなので、フォルトをさけたいなら `mlock`（RLIMIT_MEMLOCK が数える）[A 16, 22]。
- futex: 共有の futex は `FUTEX_PRIVATE_FLAG` をつけない。返り値 `EAGAIN`（もう変わっていた）・`ETIMEDOUT`・`EINTR`、0 でも値を見なおす [A 15]。Rust の `linux-futex`（BSD-2-Clause、`Shared` 型）[A 28、入れる版は未確認]。
- eventfd は poll に入れられ、パイプより軽い。PipeWire 自身は eventfd と memfd の activation でクライアントを起こす [A 17, 25]。
- リアルタイム: fork した子はスケジューリングを受けつぐが、`SCHED_RESET_ON_FORK` なら受けつがない [A 20]。`RLIMIT_RTTIME` はブロックせずに走れる時間の上限で、こえると SIGXCPU ・ SIGKILL [A 19]。PipeWire の module-rt は RLIMIT_RTPRIO が足りなければ rtkit か portal に頼む [A 24]。SCHED_FIFO が取れない子はふつうのスケジューラで動き、起きるのが遅れると xrun になる [C]。

### 見はり
- 前の検証（パイプ）: 2 ブロック返事なしで 10.8 ms、kill は 3.2 ms（パイプ切れ）、segv はコアダンプを書く約 1 秒のあいだパイプが開いたままで、見はりが先に気づいた（`docs/report.md` 3-2）。
- `pidfd_open`（5.3〜）で子の終わりを poll で知る。`pidfd_send_signal` で pid の使いまわしの事故をさける [A 18]。`PR_SET_PDEATHSIG` の「親」は子を作った**スレッド**で、そのスレッドが終わるとシグナルが来る [A 21]。
- 子が書きかけで死んだとき（設計案 [C]）: 子が「書き終わり」の印（ブロックの通し番号）を立てたあとだけ読む。子が書いた長さや輪の位置は信用せず範囲を調べる。NaN は和の所で止める（meloet の決まり）。

### やりとりの中身（Carla の分けかた [A 1]）
| 中身 | 道 |
|---|---|
| 音 | 共有メモリ |
| MIDI、ホスト → プラグインのパラメータ | 音のスレッドの輪 |
| プラグイン → ホストのパラメータ、窓、名前 | 音でない輪かソケット |
| 再生の位置とテンポ | 音のスレッドの共有領域の構造体。起こす前に親が書く |
| 状態の保存（大きい） | 別の道（Carla はファイル ＋ 返事、60 秒） |

輪の作法: 書き手 1 人・読み手 1 人。書く側は中身を書いてから release で head を進め、読む側は acquire で head を読む。大きさは 2 のべき乗 [A 26]。`rtrb` 0.4.1（MIT/Apache-2.0）は同じプロセスの中向けで、プロセスをまたぐ使いかたは書いていない [A 27]。

### プロセスの数とメモリ
- 空のプロセス、GUI のライブラリを読んだプロセス、Surge XT 1 つのメモリ、プロセスの立ち上げの時間は、**使える測定が見つからない**。この PC で PSS（`smaps_rollup`）で測る。
- 16 トラック × （重いシンセ 1 ＋ エフェクト 4）＝ 80 個の見つもり [C]: 1 つずつなら 80 プロセス（rtkit の 15 をこえる）、同じプラグインでまとめれば約 5、トラックごとなら 16、全部まとめれば 1。80 回を直列に往復すると、1 回 4.3 µs なら 0.34 ms、30 µs なら 2.4 ms（1 ブロック 5.33 ms）。

## 調べ物の係の提案（C）
- プロセスの分けかたは「えらべる」にして、はじめは「同じプラグインでまとめる」。落ちたことのある物と、人が決めた物は「1 つずつ」へ。窓と DSP は同じプロセス。
- 音は同じブロックで往復。待ちの上限はブロックより短く。前のブロックの返事がまだなら待たずにとばす。子は先に全員起こしてから、まとめて待つ。子は自分でリアルタイムをもらう。
- 起こしかたは futex。見はりは 2 段（音のスレッドで 2 ブロック、見はり役のスレッドで長く）。止めるのは `pidfd_send_signal(SIGKILL)`、死んだのは pidfd の poll で知る。
- 子の fd 1 ・ 2 は起動の前にファイルか `/dev/null` へ。

## わからなかった事
Bitwig と REAPER の中の作り ・ 遅れの数、Bitwig のはじめの値、プロセスのメモリと立ち上げの時間、futex と eventfd の同じ条件の往復、rtkit の `MakeThreadRealtimeWithPID` が子をどう見るか、Ubuntu 26.04 の rtkit の値。

## 出どころ
| # | URL | 確かさ |
|---|---|---|
| 1 | https://raw.githubusercontent.com/falkTX/Carla/main/source/utils/CarlaBridgeUtils.hpp | A |
| 2 | https://raw.githubusercontent.com/falkTX/Carla/main/source/utils/CarlaBridgeUtils.cpp | A |
| 3 | https://raw.githubusercontent.com/falkTX/Carla/main/source/utils/CarlaSemUtils.hpp | A |
| 4 | https://raw.githubusercontent.com/falkTX/Carla/main/source/backend/plugin/CarlaPluginBridge.cpp | A |
| 5 | https://raw.githubusercontent.com/falkTX/Carla/main/source/utils/CarlaRingBuffer.hpp | A（解釈は B〜C） |
| 6 | https://raw.githubusercontent.com/robbert-vdh/yabridge/master/docs/architecture.md | A |
| 7 | https://raw.githubusercontent.com/robbert-vdh/yabridge/master/CHANGELOG.md | A |
| 8 | https://raw.githubusercontent.com/robbert-vdh/yabridge/master/README.md | A |
| 9 | https://bitwig.com/userguide/latest/vst_plug-in_handling_and_options | A |
| 10 | https://www.bitwig.com/en/18/bitwig-studio-2_5.html | A（2019 年） |
| 11 | https://dlx.reaper.fm/userguide/html/（16.18 章、検索の要約経由） | B |
| 13 | https://ardour.org/plugins-in-process.html | A（数は仮定） |
| 14 | https://manual.zrythm.org/hu/plugins-files/plugins/plugin-window.html | A |
| 15 | https://man7.org/linux/man-pages/man2/futex.2.html | A |
| 16 | https://man7.org/linux/man-pages/man2/memfd_create.2.html | A |
| 17 | https://man7.org/linux/man-pages/man2/eventfd.2.html | A |
| 18 | https://man7.org/linux/man-pages/man2/pidfd_open.2.html、pidfd_send_signal.2、pidfd_getfd.2 | A |
| 19 | https://man7.org/linux/man-pages/man2/getrlimit.2.html | A |
| 20 | https://man7.org/linux/man-pages/man7/sched.7.html | A |
| 21 | https://man7.org/linux/man-pages/man2/PR_SET_PDEATHSIG.2const.html | A |
| 22 | https://man7.org/linux/man-pages/man2/mmap.2.html | A |
| 23 | https://raw.githubusercontent.com/heftig/rtkit/master/README、rtkit-daemon.c | A（配布元で変わりうる） |
| 24 | https://docs.pipewire.org/page_module_rt.html | A |
| 25 | https://docs.pipewire.org/page_scheduling.html | A |
| 26 | https://www.kernel.org/doc/html/latest/core-api/circular-buffers.html | A |
| 27 | https://docs.rs/rtrb/latest/rtrb/ | A |
| 28 | https://docs.rs/linux-futex/latest/linux_futex/、https://docs.rs/atomic-wait/latest/atomic_wait/ | A（入れる版は未確認） |
| 30 | https://kamalmarhubi.com/blog/2015/06/10/some-early-linux-ipc-latency-data | C |
| 31 | https://eli.thegreenplace.net/2018/measuring-context-switching-and-memory-overheads-for-linux-threads/ | C |
| 34 | https://github.com/robbert-vdh/yabridge/releases/tag/3.4.0 | A |
| 35 | https://git.kx.studio/jackaudio/jack2/src/branch/master/linux/JackLinuxFutex.h（検索の要約経由） | B |
