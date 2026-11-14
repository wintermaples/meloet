# 途中のまとめ（2026-11-14 時点）

最終の報告書の前に、作りながら気づいたことを書きとめたものです。数字は本番の計測前の「試走」の値をふくみます。最終の数字は `docs/report.md` に書きます。
各項目の末尾の [A] [B] [C] は確かさの度合いです（A＝一次情報や自分の計測、B＝一次情報を引いた二次情報、C＝推測）。根拠の一覧は最後にあります。

---

## 1. このPC

| 項目 | 値 |
|---|---|
| OS | Ubuntu 26.04.1 LTS（Ubuntu Studio のパッケージ群が入っている）、カーネル 7.0.0-43 |
| GPU | NVIDIA GeForce RTX 5060 Ti、オープン版カーネルモジュール 595.99.02。ほかに AMD Raphael（内蔵）。画面は 2 枚とも NVIDIA 側 |
| 画面 | 2560×1440 が 2 枚、どちらも約 144 Hz（143.87 / 143.91）。1 フレーム ≒ 6.95 ms |
| デスクトップ | GNOME 50.1、Wayland。Xwayland あり |
| WebKitGTK | 2.52.6-0ubuntu0.26.04.1（webkit2gtk-4.1） |
| PipeWire | 1.6.2 |
| 音の出口 | Ryzen HD Audio（ALC897 アナログ）、モニタの HDMI |
| MIDI | 調べた時点では USB の鍵盤はつながっていなかった（`lsusb` と ALSA シーケンサのどちらにも出ていない） |

[A] 自分で読んだ値。`spikes/s1-render/results/env.txt` に記録。

---

## 2. 計測する環境の都合（本番のユーザーには関係しないこと）

わたしは `meloet-agent` という別ユーザーで動いていて、デスクトップは `wintermaples` さんのものです。そのせいで起きたことを分けて書きます。数字をまちがって読まないためです。

- **画面と音に入れなかった。** ACL と `xhost` で一時的に入口をあけてもらいました。ログアウトで消えます。[A]
- **起動のたびに 30 秒止まった。** わたしのユーザーの `xdg-desktop-portal` が起動に失敗していて、GTK が D-Bus の応答を 25 秒＋5 秒待っていました（strace と journal で確認）。計測では、プロセスごとに `dbus-run-session` で新しい D-Bus を用意して回避しました。起動は 31.7 秒から 1.8 秒になりました。本当のユーザーのセッションでは portal が動くので、この待ちは起きないはずです。[A]（原因）／[C]（本番で起きないという見こみ）
- **わたしの環境には `GDK_BACKEND=wayland` がもともと入っていた。** X11 で試すときは明示的に上書きしています。[A]

---

## 3. 賭け1（画面の速さ）でわかったこと

### 3-1. WebKitGTK は 144 Hz の画面でも 60 Hz でしか描かない ← いちばん大事

- `requestAnimationFrame` の間隔が、どの設定でも 16〜17 ms でした。WebGL の描画の仕事が 1 ms 未満のときでも同じです。[A]
- 原因の流れ（ソースと実験で確認）：
  1. WebKit は、DRM の `drmWaitVBlank` で画面の書きかえの合図を取ろうとする（`DisplayVBlankMonitorDRM.cpp`）。[A]
  2. NVIDIA（`/dev/dri/card1`）は `DRM_IOCTL_WAIT_VBLANK` に **ENOTSUP** を返す。ioctl を直接たたく検査（`spikes/s1-render/tools/vblank_probe.py`）で確認。[A]
  3. 失敗すると、WebKit は**固定 60 Hz のタイマー**（`DisplayVBlankMonitorTimer`）に切りかえる。journal に `Failed to create DRM vblank monitor, falling back to timer` と出る。[A]
- journal のメッセージは「許可されていない操作（EPERM）」と言っていますが、これは WebKit 側の表示の誤りと見ています。`drmWaitVBlank()` は -1 を返して errno を立てるので、`strerror(-ret)` だといつも EPERM と表示されます。本当の原因は ENOTSUP です。[A]（ioctl の結果）／[C]（ログの誤りという解釈）
- ENOTSUP は装置そのものの返事で、ユーザーの権限とは関係しません。なので、ヤチヨさん本人のユーザーで動かしても同じになる見こみが高いです（本人のユーザーでの確認は、まだしていません）。[C]
- WebKit の `PreferPageRenderingUpdatesNear60FPS` は最初からオンでした。切っても 60 Hz のままだったので、原因ではありません。[A]
- **意味すること**：仕様の合格ライン「1920×1080 で、フレームの 95% が 1 回の書きかえ（6.95 ms）におさまる」は、このPCの WebKitGTK では**描画の重さに関係なく、しくみの上で届きません**。最終の報告では、厳しい基準（144 Hz）と、WebKit が実際に回している 60 Hz の両方で数字を出し、合否は厳しいほうで書きます。

### 3-2. NVIDIA と DMABuf

- Ubuntu の WebKitGTK には、Debian から来た `disable-nvidia-dmabuf.patch` が入っています。NVIDIA を見つけると、**何もしなくても DMABuf の描画を切ります**（Launchpad のパッチ本文で確認）。だから公式ページにある `WEBKIT_DISABLE_DMABUF_RENDERER=1` は、このPCでは何も変えないはずです（本番の計測で確かめ中）。[A]
- 逆に `WEBKIT_FORCE_DMABUF_RENDERER=1` で強制的に戻すと：
  - Wayland：**起動直後に落ちる**。`Gdk-Message: Error 71 (Protocol error)`。Tauri 公式の Linux Graphics ページの症状そのもの。[A]
  - そこに `__NV_DISABLE_EXPLICIT_SYNC=1` を足すと動く。ただし 60 Hz のまま。[A]
  - `GDK_BACKEND=x11` のときは**固まる**（`Failed to create GBM buffer ... 無効な引数です`）。[A]
- WebGL のレンダラ名は `Apple GPU` と出ます。公式ページに書いてあるとおりで、GPU で描いているかを JS からは見分けられません。[A]
- 全部の設定を並べた本番の計測は、いま回しています。

### 3-3. 試走の数字（1280×800、メーターなし、3 秒）

| 描き方 | rAF の間隔 p50 / p95 | 描く仕事の平均 / p95 |
|---|---|---|
| WebGL（インスタンス描画、1 回で全部） | 16 / 17 ms | 0.4 / 1 ms |
| Canvas 2D（見えている所だけ） | 16 / 23〜24 ms | 7.3 / 21〜22 ms |

Canvas 2D は、拡大縮小で 1 万ノートが全部見えるときに 20 ms を超え、フレームを落とします。WebGL は余裕があります。[A]（試走。本番の数字は最終の報告書で）

### 3-4. 計測のしかたの限界

- WebKitGTK の `performance.now()` は **1 ms 刻み**です。COOP/COEP のヘッダを Tauri の設定で足しても `crossOriginIsolated` は false のままで、細かくなりませんでした。[A]
- フレームの抜けは「7 ms 前後か 14 ms 以上か」「16 ms か 33 ms か」で見分けられるので、合否の判定には使えます。描く仕事の時間の p95 などは、1 ms の粗さをふくみます。[A]
- 「まっ白」「ちらつき」は、ページの中からは測れません。あとで目で見てもらう予定です。

---

## 4. 賭け4（鍵盤の返事）に向けてわかったこと

- **PipeWire の `clock.force-quantum` が 1024 に固定されていました。** このままだと、アプリが 256 を頼んでも 1024 で動きます。Ubuntu Studio の設定ファイルには見当たらなかったので、どこで決まっているかは未確認です。計測のあいだだけ 256 にする許可をもらいました。[A]（値）／[C]（出どころ）
- PipeWire のノード名は「Ryzen HD Audio Controller アナログステレオ」のような装置の名前です。「スピーカー」「ヘッドホン」という人の言葉は、そのままでは取れません。cpal 0.18.2 には `DeviceDescription::device_type()`（Speaker / Headphones などの種類）がありますが、PipeWire でちゃんと値が入るかは実機で確認中です。[A]（API）／未確認（値）
- cpal の PipeWire 側は、時刻に `CLOCK_MONOTONIC` をそのまま使っています（`pw_stream_get_time_n` と同じ時計）。「Rust が鍵を受けた時刻」と「DAC に届く予定の時刻」を、同じ時計の上で引き算できます。[A]（ソース）
- **midir には鍵盤の抜きさしを知らせる仕組みがありません。** ALSA シーケンサのお知らせポート（System:Announce）を、`alsa` クレートで直接購読する作りにしました。[A]
- **cpal 0.18.2 は `alsa` クレートの 0.11 系を使っています。** 同じ native ライブラリを二重にはつなげないので、こちらも 0.11 にそろえる必要がありました（0.12 だとビルドの依存解決で止まる）。[A]

---

## 5. 賭け2・3（プラグイン）に向けてわかったこと

### 5-1. LV2 の窓の型（このPCに入っている TTL から読んだ値）

| プラグイン | 窓の型 | 窓に必要な機能 | メモ |
|---|---|---|---|
| Surge XT（LV2） | X11UI | `instance-access`（必須）、`idleInterface`、`resize` | 窓が DSP を直接さわる |
| synthv1 | X11UI ＋ Qt6UI ＋ kx 外部窓 | `instance-access`（必須） | 型を 3 つ持つ |
| Dragonfly Reverb（4 種） | X11UI | `options`（必須）、`idleInterface`、`showInterface` | `instance-access` は不要 |
| LSP Plugins（194 個） | X11UI | `urid:map`（必須）、`instance-access` は任意 | 全部同じ形 |

[A]

- **4 つとも X11UI で開ける**ので、最初の版は「X11UI だけ」で全員の面倒を見られる見こみです。[C]（実際に開けるかは試験中）
- Surge と synthv1 は `instance-access` が必須です。つまり**窓は DSP と同じプロセスに置く必要があります**。「プラグインを読む層を別プロセスに出す」なら、窓もその別プロセスの側に置く、という形に決まります。[A]（TTL の宣言）／[C]（設計への影響）
- システムの suil 0.10.24 には `x11`、`x11_in_gtk3`、`x11_in_qt5` の 3 つの型が入っています。Gtk2UI の型はありません。[A]

### 5-2. VST3

- `vst3` 0.3.0（MIT OR Apache-2.0）は、VST3 SDK のヘッダから作ったバインディングをクレートの中に持っています。SDK がなくてもビルドできます。中は unsafe で、COM の扱い（`ComPtr`、`ComWrapper`）だけが用意されています。[A]
- VST3 SDK は 3.8 から MIT です。[A]
- Linux の窓は X11 の XEmbed が前提です。ホストは `Linux::IRunLoop`（fd の監視とタイマー）を用意する必要があります。3.8 には Wayland のプレビュー（`IWaylandHost`）もあり、バインディングにも入っていました。[A]
- Surge XT は JUCE 製で、部品とつまみ係のあいだのやりとりに `IMessage` と `IAttributeList` を使うはずなので、ホストの側で用意しました。[C]（記憶。実機で確かめる）
- `lsp-plugins-vst3` は Ubuntu 26.04 の apt で入りました（1.2.27。backports には 1.2.35）。24.04 の本体のリポジトリにはありません。[A]

### 5-3. `vst3-host` クレート（MIT）をお手本として読んだ結果

- 別プロセスで動かすとき、**音のバッファを JSON（base64）にして stdin/stdout で送っています**。安全のための作りとしてはわかりやすいです。ただ、256 サンプル（5.3 ms）ごとに間に合わせる必要がある本番の音の道には向きません。[A]（ソース）／[C]（向かないという判断）
- Linux の窓は Xvfb の上でしか試していない、と本人の文書にあります。[A]
- 0.11.1 で、5 か月に 17 回出ていて、動きがとても速いです。egui・midir・cpal を抱える作りでもあります。[A]
- **判断：依存には入れない。** `IRunLoop` の回しかたと COM の実装の形は参考にしました。[C]

### 5-4. LV2 を読む部品

- `livi` 0.9.0（MIT）＋ システムの `lilv`（ISC）を土台にしました。起動・処理・つまみ・状態の保存と復元がそろっています。ただし窓の機能はないので、UI の `.so` を読む部分は自分で書きました。[A]
- 純 Rust で、ゆるいライセンスで、十分に育った LV2 ホストは見つかりませんでした（`yeli` は LGPL-2.1+ で静的リンクになるので外しました）。[B]

---

## 6. 部品の版について気づいたこと

- Tauri の最新の安定版は 2.12.5 です。3.0 は alpha.6 まで出ていて、3 系には CEF で動かす部品（`tauri-runtime-cef`）があります。60 Hz の問題の逃げ道の候補になりえます。[A]
- Tauri 公式の Svelte テンプレートは SvelteKit（adapter-static）です。試作では、薄くするために素の Vite ＋ Svelte にしました。本番でどちらにするかは提案で書きます。[A]
- 版はすべて `=` で固定しています（tauri 2.12.5、tauri-build 2.7.4、cpal 0.18.2、vst3 0.3.0、livi 0.9.0、svelte 5.57.3、vite 8.3.8 など）。[A]

---

## 7. まだわかっていないこと・これからやること

- 賭け1：全部の設定の本番の計測（いま回している）、窓の大きさを変え続けても落ちないか、まっ白・ちらつきの目視
- 賭け2・3：ビルドと実機での試験（音・つまみ・状態・窓・落ちたとき）
- 賭け4：鍵盤のさし・ぬき、鍵 → 画面、鍵 → 音、バッファ 256、送り方 4 種の比較
- ヤチヨさん本人のユーザーでの 60 Hz の確認（コマンド 1 つで済む予定）

---

## 根拠の一覧

| 根拠 | 確かさ |
|---|---|
| 自分の計測と検査（`spikes/s1-render/results/`、`vblank_probe.py`、strace、journal） | A |
| WebKitGTK 2.52.6 のソース（GitHub の `webkitgtk-2.52.6` タグの `DisplayVBlankMonitor*.cpp`） | A |
| Ubuntu の `disable-nvidia-dmabuf.patch`（Launchpad の git） | A |
| Tauri 公式「Linux Graphics Issues」「Prerequisites」「Calling the Frontend」 | A |
| tauri 2.12.5、tauri-utils、tao、cpal 0.18.2、vst3 0.3.0、livi 0.9.0、alsa 0.11 のソース（`~/.cargo/registry`） | A |
| Steinberg VST3 開発者ポータル（ライセンス、IRunLoop、プラットフォームの型） | A |
| LV2 の TTL（`/usr/lib/lv2/*`）、パッケージの copyright ファイル | A |
| `vst3-host` 0.11.1 のソースと README | A |
| JUCE 製プラグインが IMessage を使うこと | C（記憶。実機の試験で確かめる） |
| 純 Rust の LV2 ホストが見つからないこと | B（crates.io の検索結果。網羅の保証はない） |
| 本人のユーザーでも 60 Hz になる見こみ、各種の設計判断 | C（推論） |
