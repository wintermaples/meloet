# 配る形（Tauri 2 の bundler、AppImage、Flatpak）

2026-11-22 調べ。置き場の案は `docs/research/2026-11-22-distribution-formats.md`（私は書きこみません。棟梁が置いてください）。確かさは A 一次、B 二次、C 推論や個人の投稿。

## 1. 結論（短く）

- **主力は deb。** Ubuntu 24.04 の上で作る。ホストの WebKitGTK、Mesa、NVIDIA の EGL をそのまま使えるので、下の AppImage の EGL 事故を避けられる。プラグイン係も本体と同じ `/usr/bin` に置ける。（確かさ A）
- **AppImage は「おまけ」にとどめる。** 70 MB 以上になる。WebKit の補助プロセスと Wayland などの部品を同じ袋に入れるため、ホストの Mesa とずれる事故が 2026 年に何度も出た（CLI 2.12 で直った、とされるが、直しのあとにも別の報告が残る）。（A）
- **Flatpak は選ばない。** Flatpak の中からは、ホストの `/usr/lib/vst3` のプラグインを実用の形では読めない。Flatpak 側の人が「ホストのコードをサンドボックスの中に入れるのは、やる気がない」と言っている。Bitwig の公式も「`/usr/lib` に入れたものは Flatpak 版から見えない」と書く。（A）
- deb の `Depends` は、自分で書く。公式の文書は「bundler が WebKitGTK と GTK を自動で足す」と書くが、ソースの `debian.rs` は設定の `depends` だけを書き出していた。食いちがいがある（高いほう＝ソースを採る）。できた deb を `dpkg-deb -I` で見て確かめる。（A）
- 入れる tauri-cli は 2.12 以上の新しい物（最新は 2.12.6、releases ページ。B、年が出ていない）。AppImage の直しは 2.12 から。

## 2. 質問ごとの中身

### 2-1. Tauri 2 の bundler が Linux で作れる形

`bundle.targets` は `deb`、`rpm`、`appimage`、`pacman`（設定の文書。A）。配る方法の案内ページには、deb、Snap、AppImage、Flatpak、RPM、AUR がある（A）。

| 形 | 入れかた・消しかた | 更新 | 依存 | 弱い所 |
|---|---|---|---|---|
| deb | `apt install ./x.deb`、`apt remove` | 自動の更新なし。apt の置き場（PPA や自前）を立てれば apt upgrade で（C、今回は確かめていない） | 設定の `bundle.linux.deb.depends` だけが `Depends:` になる（ソース。A）。`recommends`、`conflicts`、`replaces`、保守スクリプトも書ける | glibc は作った PC で決まる（下の注）。置き場を持たなければ、人が手で入れなおす |
| AppImage | `chmod a+x` して動かすだけ。消すのはファイルを消す | 自動なし | 部品をまるごと入れる | 大きい（文書は「2〜6 MB が 70 MB 以上に」）。Mesa や NVIDIA との食いちがい（2-2）。FUSE が要る件は今回確かめていない（C） |
| rpm | Ubuntu では使わない | ― | ソースは自動の依存を足さない | Ubuntu には無関係 |

- glibc の下限（A、Tauri の文書）: 「サポートしたい一番古い土台の上で作る」。新しい PC で作ると下限が上がり、`GLIBC_2.xx not found` になる。24.04 の glibc は 2.39、26.04 は 2.43（launchpad。A）。24.04 以上だけ相手にするなら、24.04 の上（コンテナや GitHub Actions）で作れば、24.04 と 26.04 の両方で動くはず（glibc は後ろ向きに互換。推論、C）。Tauri の文書は土台の例に 22.04 を挙げるが、meloet は 24.04 以上なので 24.04 でよい。
- deb の大きさ: WebKitGTK を入れないので小さい（文書は 2〜6 MB の目安。A）。meloet は C++ の伸縮と Rust の本体と plughost だけ。音の素材は別。

### 2-2. WebKitGTK を AppImage に入れるとどうなるか

- 文書（A）: AppImage は「依存をぜんぶ入れる」。大きさは 70 MB 以上。bundler の `linuxdeploy.rs`（A）は、`WebKitNetworkProcess`、`WebKitWebProcess`、`libwebkit2gtkinjectedbundle.so` を明示してコピーする。残りの共有ライブラリは linuxdeploy が拾う。`--plugin gtk` は必ず付く。
- 事故（Tauri 本家の issue。A）:
  - #15976: Fedora 44（Mesa 26）で、Ubuntu 22.04 の上で作った AppImage が `Could not create default EGL display: EGL_BAD_PARAMETER` で落ちる。袋に入った `libwayland-*`、`libxkbcommon`、`libxcb-*` などが `LD_LIBRARY_PATH` でホストの物より先に読まれ、新しい Mesa とずれる。
  - #15665: Ubuntu 26.04（Mesa 25、GLib 2.88）でも同じ。GLib 系、GStreamer の部品まで入りすぎる、との報告。Tauri の人（FabianLars）は「linuxdeploy を更新して libwayland を除くたびに、AppImage がもっとこわれた」と言う。
  - 直し: PR #16062（2026-09-25 に merge、2.12 の予定）。linuxdeploy とスクリプトを更新。ただし同じ PR の議論に、Fedora や openSUSE での https（gnutls）、`GIO_MODULE_DIR` が起こした子のプロセスにもれる、openSUSE の bundle のこわれ、SVG が出ない、という「直っていない所」が残った（A）。
  - 実験の形: PR #12491「truly portable AppImage」（ホストのグラフィック部品を使う）は下書きのまま、2.13 の予定（A）。いまは製品に使えない。
- NVIDIA まわり（A、Tauri の Linux graphics ページ）: DMABUF の描画が NVIDIA でこわれる事がある。`WEBKIT_DISABLE_DMABUF_RENDERER=1`、`__NV_DISABLE_EXPLICIT_SYNC=1`、`WEBKIT_DISABLE_COMPOSITING_MODE=1` の順に試す。「影響を確かめてから出す」と書いてある。WebGL は「ソフトの道でも作れてしまい、エラーが出ない」ので、WebGL を多く使う画面は気をつける、とある。meloet はまさにこれに当たる。AppImage 固有の EGL 事故では、これらの変数が効かなかった報告がある（B、下流の issue）。
- ホストの WebKitGTK との食いちがい（推論、C）: AppImage は「作った PC の WebKit」と「ホストの Mesa や NVIDIA」を組む。deb は「ホストの WebKit」と「ホストの Mesa」で、同じ版の組。Ubuntu は WebKitGTK を版ごとあげて直す（noble と resolute の updates がどちらも 2.52.6。launchpad。A）。
- AppImage の中の `LD_LIBRARY_PATH` は、本体が起こす meloet-plughost にも伝わる。ホストの VST3 や LV2 を読むときに、袋の中の GTK や GLib が先に読まれるかもしれない（#15976 の仕組みからの推論、C）。AppImage にするなら、plughost を起こすときにこの変数を元に戻す手当てが要る。

### 2-3. 2 つめの実行ファイル（plughost）

- `bundle.externalBin`（A、sidecar の文書と `settings.rs` のソース）: 元の名前に `-<三つ組>`（例 `meloet-plughost-x86_64-unknown-linux-gnu`）を付けたファイルを用意する。bundler が三つ組を取った名前で deb と rpm の `/usr/bin/` に置く。AppImage は同じ形の `usr/` を AppDir にコピーする（`linuxdeploy.rs`、A）ので、`usr/bin/` に入るはず。
- `bundle.resources`（A）: 資料のためのもの。置き場は deb と rpm で `/usr/lib/<product_name>/`、実行の印は付かない。Rust からは `app.path().resolve(.., BaseDirectory::Resource)`。
- 文書は、探しかたに `shell().sidecar()`（shell プラグインと capability が要る）を勧める。meloet はすでに `meloet_plugin::launch::spawn` で起こすので、shell プラグインを入れるのは無駄。`std::env::current_exe()` の親のディレクトリから `meloet-plughost` を探せば、deb でも AppImage でも本体と同じ `usr/bin` にいる（推論、C）。開発の `target/` でも同じ並びで動く。
- 作りかたの注意（推論、C）: tauri-cli は、同じ workspace の別の実行ファイルを自動では作らない。`beforeBuildCommand` で `cargo build -p meloet-plughost --release` をして、三つ組の名前にコピーする。ほかの形: deb だけなら `bundle.linux.deb.files` に `"/usr/bin/meloet-plughost": "<path>"` と書ける（`files` の文書。A）。AppImage の `files` は、行き先が `/usr/` で始まる決まり（A）。

### 2-4. deb の依存の書きかた

- 名前は、24.04 も 26.04 も `t64` 付きの物が本物（launchpad。A）: `libasound2t64`（noble と resolute の両方で確認）、`libpipewire-0.3-0t64`（noble 1.0.5、resolute 1.6.2）。26.04 の `libwebkit2gtk-4.1-0` は `libgtk-3-0t64`、`libstdc++6 (>= 12)`、`libx11-6` などを自分で引く（packages.ubuntu.com の resolute。A）。
- 案（`Depends`。dpkg の「A | B」の書きかたを使う。この書きかたの文書は今回は開いていない。C）:
  `libwebkit2gtk-4.1-0, libasound2t64 | libasound2, libpipewire-0.3-0t64 | libpipewire-0.3-0, libstdc++6`
  `Recommends: pipewire, wireplumber`。
  `libasound2t64` が古い名前を `Provides` するかは、見えなかった。ただ「A | B」にすれば、どちらの名前でも通る。
- 完全な一覧は、できた実行ファイルに `ldd` や `dpkg-shlibdeps` をかけて確かめる（C、推奨）。
- 24.04 の PipeWire は 1.0.5、26.04 は 1.6.2。`pipewire` クレートがどちらの版でも動くかは、別に試す。

### 2-5. Flatpak

- ホストの `/usr` は、`--filesystem=host-os` で `/run/host` に見える（Flatpak の文書。A）。ただし「全部か無しか」で、「`/usr` は runtime のもの」。Flatpak 側の人（smcv）は、「ホストの OS のコードをサンドボックスの中の処理に入れるのは、Flatpak がやろうとしていない事」「ホストの共有ライブラリは、中にない別のホストの部品に依存してよい」と答えている（flatpak #5575。A）。つまりプラグインの `.so` が、ホストの glibc や GTK を前提にしていると、Flatpak の runtime の中では動かないことがある。実例: Bitwig の Flatpak で CLAP が新しい glibc を求めて動かない報告（B、個人の投稿）。Tauri の issue #11210 も、Flatpak の runtime の glibc より新しい glibc の deb が動かなかった例（A、別の事情だが同じ仕組み）。
- 公式の道: 拡張 `org.freedesktop.LinuxAudio.Plugins.*`。Ardour の Flathub の manifest（A）は、`/app/extensions/Plugins` に `ladspa;lv2;vst;vst3` をまとめ、`VST3_PATH` と `LV2_PATH` をそこに向ける。拡張の branch は runtime の版（Ardour は 26.08、Bitwig は 25.08）に合わせる決まり（Flatpak の文書。A）。ホストの `apt` で入れたプラグインは、この道では見えない。
- ALSA シーケンサ: Bitwig と Ardour の manifest は `--device=all`（A）。Flatpak の文書は `--socket=pulseaudio` に「MIDI と `/dev/snd` の ALSA」を含む、とも書く（A）。`/dev/snd/seq` だけを許す細い設定は見つからなかった。PipeWire: Bitwig は PipeWire のソケット、Ardour は `xdg-run/pipewire-0:ro`（A）。
- リアルタイム: portal の `org.freedesktop.portal.Realtime` が rtkit に取りつぐ。rtkit の上限は越えない（A）。PipeWire は portal が使えれば使う（B）。
- Ubuntu は Flatpak を最初から入れていない。`apt install flatpak` と Flathub の追加が要る（Ubuntu Wiki、Flathub のページ。B）。meloet のおもな読者（はじめての人）には、手が増える。
- 選ばない理由（まとめ）: (1) 人が持っている VST3 と LV2 が見えない、(2) 見えても glibc などでずれる、(3) X11 の窓（plughost）に `--socket=x11` や `--device=all` を足すと、サンドボックスの意味がうすい、(4) Ubuntu で入れる手間。

### 2-6. ほかの DAW の配りかた

| DAW | 配る形（公式） | Flatpak でのプラグイン |
|---|---|---|
| Ardour | 公式の `.run` インストーラ（自分のライブラリを持つ。A）、ディストロのパッケージ。Flathub 版は「有志の物、公式とは無関係」と表示（A） | 開発者の発言: 公式の `.run` は影響なし、Flatpak では Flatpak のプラグインを使う（B。tracker の本文は開けず、検索の要約と forum の Paul Davis の発言から。「公式は Flatpak を出さない」の一文はこの要約の中のもの） |
| Bitwig Studio | Flatpak と Ubuntu 向け（24.04 以上。A） | 公式サポート: 「Flatpak で入れたプラグインを勧める。`/usr/lib` は見えない」（A） |
| REAPER | `.tar.xz`（x86_64 は 14 MB。A） | ― |
| LMMS | AppImage（A） | ― |
| Qtractor | ソース、AppImage、ディストロのパッケージ、Flatpak（A） | 未確認 |
| Zrythm | zip のインストーラ（有料あり。A） | 未確認。Carla でプラグインを別プロセス（A） |

### 2-7. Ubuntu Studio の事情

- プラグインの場所（A）: VST3 は `~/.vst3`、`/usr/lib/vst3`、`/usr/local/lib/vst3`、`/usr/lib64/vst3`（Steinberg）。LV2 の初期値は `~/.lv2:/usr/local/lib/lv2:/usr/lib/lv2`（lv2plug.in）。meloet の読み先と一致。
- PipeWire: Ubuntu 本体は 22.10 から標準（B）。Ubuntu Studio は 24.04 の公式ノートで PipeWire 1.0.5 が標準（A）、24.04 では「PipeWire は十分に強い」と書く。JACK 互換は初期設定で ON。
- カーネル: 24.04.3 で `lowlatency` カーネルは「公式に引退、`generic` に置きかえ」。generic は起動パラメータで低遅延の動きをする（Ubuntu Studio の発表。A）。26.04 は Linux 7.0（The Register。B）。
- 26.04 の Ubuntu Studio は Ardour 9.0.0、Carla 2.5.10 を持つ（A）。新しい利用者は、初回ログインのあとに再起動が要る（audio グループのため。A）。meloet の deb は、Ubuntu と同じに入る。

## 3. meloet へのすすめ

1. **deb を主力に。** 24.04 のコンテナで `cargo tauri build -b deb`（tauri-cli 2.12 以上）。`depends` は自分で書き、できた deb を `dpkg-deb -I` と 24.04、26.04 の素の入れ物で入れて確かめる。WebKit の更新は Ubuntu が出す。本体は `/usr/bin/meloet`、plughost は `/usr/bin/meloet-plughost`（`externalBin` か `deb.files`）。見つけかたは `current_exe()` の隣。
2. **更新の道は別に決める。** deb には自動更新がない。PPA か自前の apt の置き場が現実的（C、今回は調べていない）。
3. **AppImage は 2 つめ。** 作るなら tauri-cli 2.12.x 以上。NVIDIA と AMD、X11 と Wayland、Ubuntu 24.04 と 26.04 と Fedora で、WebGL の窓が出るか試す。plughost を起こすときは `LD_LIBRARY_PATH` を元に戻す。
4. **Flatpak は見送る。** 見直すなら、Flatpak 向けに VST3 と LV2 の拡張を作り直す大仕事になる。
5. 弱い所: deb は Ubuntu 系だけ。24.04 と 26.04 で PipeWire の版が 1.0.5 と 1.6.2 と離れている。deb の自動の依存の食いちがい（2 の結論）は、できた物で必ず確かめる。

## 4. 出どころ（見た日 2026-11-22）

| URL | 書いてあった事 | 確かさ |
|---|---|---|
| https://v2.tauri.app/distribute/ | Linux の配る形の一覧 | A |
| https://v2.tauri.app/distribute/appimage/ | 70 MB 以上、glibc は古い土台で作る、`files` は `/usr/` で始める | A |
| https://v2.tauri.app/distribute/debian/ | deb の依存、`files`、glibc の注意 | A |
| https://v2.tauri.app/develop/sidecar/ | `externalBin` の名前の決まり | A |
| https://v2.tauri.app/develop/resources/ | `resources`、`resolve` | A |
| https://v2.tauri.app/reference/config/ | `targets`、`DebConfig`、`AppImageConfig` | A |
| https://v2.tauri.app/develop/debug/linux-graphics/ | NVIDIA と DMABUF の環境変数、WebGL の注意 | A |
| https://v2.tauri.app/distribute/flatpak/ | GNOME runtime、権限、`.deb` から Flatpak | A |
| https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/linux/debian.rs | `Depends` は設定のみ、`usr/bin` と `usr/lib/<product>` | A |
| https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/linux/rpm.rs | 同じ置き場、三つ組を取る | A |
| https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/linux/appimage/linuxdeploy.rs | WebKit の補助の 3 ファイル、gtk プラグイン、AppDir の作りかた | A |
| https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/settings.rs | `copy_binaries` が三つ組を取る、`bundle_xdg_open` は 2.12 で非推奨 | A |
| https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-bundler/src/bundle/linux/freedesktop/mod.rs | .desktop とアイコンの出しかた | A |
| https://github.com/tauri-apps/tauri/issues/15976 | AppImage の EGL 事故、原因、PR #16062 で閉じた | A |
| https://github.com/tauri-apps/tauri/issues/15665 | Ubuntu 26.04 で同じ、maintainer の発言 | A |
| https://github.com/tauri-apps/tauri/pull/16062 | 直しの内容、直っていない所、merge 2026-09-25 | A |
| https://github.com/tauri-apps/tauri/pull/12491 | 実験の portable AppImage、下書き、2.13 の予定 | A |
| https://github.com/tauri-apps/tauri/issues/9662 | 「AppImage は設計上、依存を全部入れる」 | A |
| https://github.com/tauri-apps/tauri/issues/11210 | Flatpak の runtime の glibc の食いちがい | A |
| https://github.com/tauri-apps/tauri/releases | tauri-cli 2.12.6 が最新（年は出ていない） | B |
| https://docs.flatpak.org/en/latest/sandbox-permissions.html | `host-os`、`/run/host`、pulseaudio に MIDI を含む | A |
| https://github.com/flatpak/flatpak/issues/5575 | smcv: ホストのコードを入れるのは目標でない | A |
| https://docs.flatpak.org/en/latest/extension.html | 拡張の版は runtime に合わせる | A |
| https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Realtime.html | Realtime portal、rtkit の上限は越えない | A |
| https://raw.githubusercontent.com/flathub/org.ardour.Ardour/master/org.ardour.Ardour.json | Ardour の Flatpak の権限と拡張 | A |
| https://raw.githubusercontent.com/flathub/com.bitwig.BitwigStudio/master/com.bitwig.BitwigStudio.yaml | Bitwig の Flatpak の権限と拡張 | A |
| https://flathub.org/en/apps/org.ardour.Ardour | 「有志の物」、プラグインの拡張 | A |
| https://discourse.flathub.org/t/proposal-for-audio-plugins/399 | 拡張の考えかたと限界 | B |
| https://www.bitwig.com/download/ と https://www.bitwig.com/support/technical_support/52/ | Flatpak と Ubuntu、`/usr/lib` は見えない | A |
| https://ardour.org/first_time_linux.html | `.run` の入れかた | A |
| https://discourse.ardour.org/t/ardour7-in-flatpak-overcoming-the-sandboxing-with-flatseal/107984 | Paul Davis の発言 | B |
| https://www.reaper.fm/download.php、https://lmms.io/download/、https://www.qtractor.org/、https://www.zrythm.org/en/download.html | 各 DAW の配る形 | A |
| https://launchpad.net/ubuntu/+source/webkit2gtk ほか launchpad（glibc、libasound2t64、libpipewire-0.3-0t64） | 版と日付、t64 の名前 | A |
| https://packages.ubuntu.com/resolute/libwebkit2gtk-4.1-0 | 26.04 の依存の一覧 | A |
| https://discourse.ubuntu.com/t/ubuntu-studio-24-04-3-lts-released/65820 | lowlatency カーネルの引退 | A |
| https://ubuntustudio.org/ubuntu-studio-24-04-LTS-release-notes/ | PipeWire 1.0.5 が標準 | A |
| https://discourse.ubuntu.com/t/ubuntu-studio-26-04-lts-release-notes/79113 | Ardour 9.0.0、初回の再起動 | A |
| https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Plugin+Locations.html | VST3 の場所 | A |
| https://lv2plug.in/pages/filesystem-hierarchy-standard.html | LV2 の場所 | A |
| https://www.theregister.com/2026/04/24/ubuntu_resolute_raccoon/ | 26.04 は Linux 7.0 | B |
| https://wiki.ubuntu.com/Managing_software_on_Ubuntu、https://flathub.org/en/setup | Flatpak は Ubuntu に最初から入っていない | B |
| 検索の要約（個人の投稿、下流の issue） | AppImage の回避策、Bitwig の CLAP の glibc | C |

## 5. わからなかった事

- 公式の文書は deb の自動の依存を書くが、ソースは書かない。できた deb の `Depends:` を見ないと確定しない（tauri-cli 側のコードは見ていない）。
- `libasound2t64` の `Provides` と `Replaces` は、packages.ubuntu.com が開けず確かめられなかった（「A | B」の書きかたで回避）。
- Ardour の公式な「Flatpak は出さない」の一文は、bug tracker が開けず、検索の要約のみ（B）。
- `/dev/snd/seq` だけを許す Flatpak の細い設定。
- AppImage が FUSE を要する件、deb の更新（PPA、apt の置き場）の作りかた。
- 追加で調べるとよい事: tauri-cli 2.12.x の AppImage を、本物の NVIDIA、Wayland、26.04 で動かす試験。`dpkg-shlibdeps` による依存の自動の一覧。
