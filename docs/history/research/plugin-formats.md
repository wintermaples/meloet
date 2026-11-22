# プラグインの形式（VST3 ・ LV2 ・ CLAP）

調べた日 2026-11-19（meloet-research、Sonnet 5.5）。確かさは A（一次情報。この PC の `/usr/lib/lv2`・`/usr/lib/vst3`・`/usr/lib/clap` の実物を読んだ物も A）・B・C。[S番号] は最後の表。

## 結論

- **版**: `vst3` 0.3.0（2025-12-07、MIT OR Apache-2.0。VST3 SDK 3.8.0 の写し）、`livi` 0.9.0（2026-10-25、MIT）、`lilv` 0.3.1、`lilv-sys` 0.3.0、`lv2_raw` 0.3.2 は、どれもいまの最新で、検証で使った版と同じ [S1〜S4, S17〜S20] A。**VST3 SDK は 3.8.0（2025-10-20）から MIT**（GPLv3 と専用ライセンスはもう選べない）[S5〜S7] A。`vst3` クレートには Steinberg の MIT の文が入っていないので、`NOTICE` に Steinberg の著作権表示と MIT の文を足すのが安全（C）。
- **livi は音を回す所だけ**。patch:Set ・ Get、窓、instance-access、state:mapPath ・ makePath は持たない（手元のソース [S24] A）。本番に要る所は自分で書く。
- **Surge XT の LV2（1.3.4）は、つまみ 775 個をすべて `lv2:Parameter` ＋ `patch:writable` ・ `patch:readable` で出す**。control ポートのつまみは 0 [S39] A（実物）。jalv、Ardour、Carla と同じやりかたで作れる [S37, S38, S42, S66] A。同じ Surge XT でも VST3 は 2855 個（`docs/report.md` 3-1）で、**形式ごとにつまみも状態もちがう**。
- **CLAP**: `clack-host` ・ `clack-extensions` 0.2.0（2026-09-12、MIT OR Apache-2.0。README は「feature-complete、API は変わりうる」）[S44] A。土台の `clap-sys` 0.5.0。重さは VST3 の 5〜7 割の見つもり（C）。`clack-host` は `libloading =0.8` をぴったり指定している（A）。
- **同じプラグインの 3 形式をまとめる共通の ID は、どの形式にもない**。LSP は形式ごとに名前と作者の欄がちがう（A、実物）。形式ごとの正確な鍵と、見せるためだけのゆるいまとめ鍵の 2 段にする（C）。
- **遅れ**は 3 形式とも、プラグインがサンプル数で知らせる。DAW の逃げかたは Cubase（Constrain Delay Compensation）と Logic（Low Latency Mode）が公式で確かめられた。

## 1. VST3

- 0.3.0 は、書き出した束縛をクレートに入れた版（ビルドに SDK がいらない）。API の形の変更の記録はない [S2] A。0.3.0 のあとのコミットは 1 件（2026-03-28）[S3] A。
- 手元の束縛 [S4] A: `IPlugView`、`IPlugFrame`、`IRunLoop`、`IPlugViewContentScaleSupport`、`kPlatformTypeX11EmbedWindowID`、Wayland の `IWaylandHost`（3.8.0 の preview）、`IConnectionPoint`、`IComponentHandler`・`2`、`IUnitInfo`、`IMidiMapping2`、`RestartFlags::kLatencyChanged`（8）。SDK 3.8.1（2026-08-11）の `ITransportControl` はない [S8, S9] A。
- 作法（SDK のヘッダ [S10〜S13] A）: Linux はホストが `IRunLoop`（fd の `IEventHandler` とくり返しの `ITimerHandler`）を `IPlugFrame` と `IPlugFactory3::setHostContext` でわたす。大きさは `IPlugFrame::resizeView` → `onSize`。`IConnectionPoint` はホストがつなぐ（直接つながっていると思いこまない、process の最中に送らない）。`IComponentHandler` の beginEdit → performEdit → endEdit と restartComponent は UI スレッドで来る。`kLatencyChanged` ではホストが有効にしなおして聞きなおす。`kParamValuesChanged` で値を全部読みなおす。
- `moduleinfo.json`（SDK 3.7.5 から、3.7.8 から `Contents/Resources`）で、読みこまずにクラスの一覧がわかる [S14] A。LSP の VST3（190 クラス、調べるのに 8 秒）を速くできる（C）。

## 2. LV2

- livi 0.9.0 は「硬くする」版: worker の直し多数、未対応の必須機能を持つプラグインを除く、`try_build` と `FeatureError` [S22] A。
- livi がわたす機能: `urid:map` ・ `unmap`、`options`、`buf-size:boundedBlockLength`、`worker:schedule`、`state:loadDefaultState`。状態の保存はディレクトリを `None` でわたすので、ファイルを持つプラグインの `mapPath` ・ `makePath` は整わない [S24] A。`lilv` 0.3.1 は `has_latency()`、`latency_port_index()`、`uis()` を持つ [S25] A。
- 本番に要る作法 [S27〜S33] A: `ui:parent`（X11UI の親の窓）、instance-access（「強くすすめない」が Surge は必須。**DSP と窓は同じプロセス**）、`ui:idleInterface`（UI スレッドで呼ぶ。間隔は仕様にない）、`ui:resize` は非推奨（`ui:noUserResize`、`ui:fixedSize`）、options（Surge は `opts:options`、`boundedBlockLength`、`urid:map` が必須。UI に `ui:scaleFactor` と `param:sampleRate`）、worker（音のスレッドから頼み、別スレッドで動き、返事は音のスレッドへ）、`state:interface`（`threadSafeRestore` があれば音を止めずに戻せる）、`state:mapPath`（サンプラー系の前に要る）、遅れは `lv2:designation lv2:latency`（`lv2:reportsLatency` は古い）。

## 3. patch:Set ・ patch:Get（Surge XT の LV2）

- 実物 [S39] A: 例 `a lv2:Parameter ; rdfs:label "Bypass Surge XT" ; pg:group plug:paramgroup_Root ; rdfs:range atom:Float ; lv2:default 0 ; lv2:minimum 0 ; lv2:maximum 1`。775 個すべて `atom:Float`。中身のない名前（"M1: -"）や重なる名前（"FX A1 -"）がある。入力の atom ポート `in`（`atom:Sequence`、midi ・ patch:Message ・ time:Position、`lv2:designation lv2:control`、`rsz:minimumSize 63992`）、出力の atom ポート `out`（patch:Message、63992）、遅れの control 出力ポート `latency`。種類は `lv2:InstrumentPlugin`。窓は X11UI で、`ui:idleInterface`、`urid:map`、`ui:parent`、`instance-access` が必須。
- 仕様（patch 2.10 [S30]、parameters 1.4 [S31] A）: `patch:Set` は 1 つの property に 1 つの値。`patch:Get` に property があれば返事はその Set。`patch:sequenceNumber` で返事をひもづける。
- ホストのやりかた: jalv は writable と readable を列挙し、`rdfs:range` を Int ・ Long ・ Float ・ Double ・ Bool ・ String ・ Path の順に当て、範囲がなければ 0〜1、`patch:Set` を入力ポートに入れ、空の `patch:Get` を 1 度送り、出力ポートの物は種類を問わず「変わった property」として読む [S37, S38] A。Ardour と Carla も同じ形 [S66, S42] A。
- 作るときの目安（C）: `rdfs:range` が Float ・ Int ・ Bool ・ Double の物をパラメータにする。`pg:group` をグループの名前に。1 回の process で同じ property は最後の値だけ入れる。出力の atom は `rsz:minimumSize` 以上で取る（足りないと Carla で困った報告 [S41] B）。はじめと状態を戻した直後に空の `patch:Get`。

## 4. CLAP

- 作法（ヘッダ [S47] A）: 共有ライブラリが `clap_entry` を出す（`init` は 1 度、速く、窓を出さない）。記述子は `id`（逆ドメイン）、`name`、`vendor`、`url`、`version`、`description`、`features`。`init`（主）→ `activate`（主。遅れと入出力の形は deactivate までそのまま）→ `start_processing` ・ `process`（音）→ `stop_processing` → `deactivate` → `destroy`。state の save ・ load は主スレッド。gui は X11 が XEmbed、Wayland は浮く窓。latency の `changed()` は有効化の最中だけ、有効なら `request_restart()`。timer-support（主スレッド、ホストは周期を直してよい、30 Hz は許すべき）、posix-fd-support（レベルトリガー）。
- 重さの見つもり（VST3 を 1.0 として。C）: スキャン 0.05、params 0.1、state 0.05、ポートと process 0.1、latency 0.05、窓 0.1〜0.2、timer と fd の回し役と thread-check 0.1（VST3 の IRunLoop の仕掛けを使いまわす）、`request_callback` ・ `request_restart` ・ `request_process` の受け皿 0.05〜0.1。合計 **0.5〜0.7**。patch を除いた LV2 より軽く、patch をふくむ LV2 と同じくらい。
- この PC の実物 [S62] A: `/usr/lib/clap` に 20 個（Surge XT、Surge XT Effects、Zam 系）。Surge XT の id は `org.surge-synth-team.surge-xt`、features は instrument ・ synthesizer ・ stereo、vendor は "Surge Synth Team"。LSP の CLAP は入っていない。
- CLAP の版は `main` の `version.h` が 1.2.10、REAPER は 1.2.7 に更新（2026-04）[S47, S49] A。

## 5. 遅れ（レイテンシ）

| 形式 | 知らせかた | 動いている最中に変わったとき |
|---|---|---|
| VST3 | `IAudioProcessor::getLatencySamples()` | `restartComponent(kLatencyChanged)`。ホストが有効にしなおして聞きなおす [S11, S12] A |
| LV2 | `lv2:designation lv2:latency` の control 出力ポート（古くは `lv2:reportsLatency`）[S26] A | 手続きなし。ポートの値を毎ブロック読む（jalv）[S37] A |
| CLAP | `clap_plugin_latency::get()` | `changed()` は有効化の最中だけ。有効なら `request_restart()` [S47] A |

| DAW | 弾いている最中の逃がしかた | 確かさ |
|---|---|---|
| Cubase | Constrain Delay Compensation: 録音待ちのトラック ・ 楽器 ・ グループ ・ 出力のプラグインのうち、しきい値をこえる物を止める（「Live」ボタンのある VST3 はそのモード）。録音のあと戻すよう案内 | B [S55] |
| Logic | Low Latency Mode: トラックの合計の遅れが Limit をこえないよう、遅れの大きいプラグインを回りこむ | A [S56] |
| Ardour | 再生側を読み先行で合わせる。プラグインの遅れを監視中に逃がす機能は見つからない | A ・ C [S57] |
| REAPER ・ Bitwig | 一次情報では見つからない（REAPER は第三者の ReaLlm） | B ・ C |

- 調べ物の係の案（C）: 案 A「いつも全部合わせて、遅れを数で見せる」、案 B「Cubase ・ Logic 型の自動で外す」、案 C「案 A ＋ ライブ向きの目印」。推しは A に C を足し、B は測って要ったら。

## 6. 同じプラグインをまとめる

- 各形式の情報 [S14, S26, S47] A: VST3 は cid（128 ビット）、`PClassInfo2` の name ・ vendor ・ version ・ subCategories。クラスは Audio Module Class と Component Controller Class が別に並ぶ。LV2 は URI（変えてはいけない）、`doap:name`、`doap:maintainer` ・ `doap:developer` の `foaf:name`、`lv2:project`。CLAP は id ・ name ・ vendor ・ version ・ features。
- 実物 [S62] A:

| プラグイン | 名前 | 作った人 |
|---|---|---|
| Surge XT（3 形式） | 3 つとも "Surge XT" | 3 つとも "Surge Synth Team" |
| ZamComp（DPF、3 形式） | 3 つとも "ZamComp" | 3 つとも "Damien Zammit" |
| LSP Compressor Stereo（LV2 ・ VST3） | LV2 "LSP Compressor Stereo"、VST3 "Compressor Stereo" | LV2 の maintainer "LSP LV2"（developer "Vladimir Sadovnikov"）、VST3 の vendor "LSP VST3" |

- まちがえてまとめる例: "Surge XT" と "Surge XT Effects"（前方一致）、LSP の "Compressor Stereo" と "Sidechain Compressor Stereo" ・ "GOTT Compressor Stereo"。まとめそこなう例: LSP の LV2 と VST3。
- 作る道具: DPF は LV2 の URI と CLAP の id を別々に決める [S63] A。JUCE は `LV2URI` を `COMPANY_WEBSITE` と `PLUGIN_NAME` から作り、形式をまたぐ ID はない [S64] A。
- ホスト: Ardour は VST3 があれば VST2 を、LV2 があれば LADSPA を隠す（LV2 と VST3 は互いに隠さない）[S60] A。開発者は「名前だけでは同じとは言えない」[S61] B。REAPER は 6.71 で「形式の重なりを隠す」を改善 [S50] B。
- 鍵の案（C）: 曲の中の鍵は `vst3:<Audio Module Class の cid>`、`lv2:<URI>`、`clap:<id>` だけ。まとめ鍵（見せるだけ）は「種類（音源 ・ エフェクト）＋ 整えた名前 ＋ 整えた作者」。名前は小文字にして形式の名や末尾の "(Stereo)" を外す、作者は小文字にして末尾の "VST3" ・ "LV2" を外す。名前が一致して作者も一致すればまとめる。前方一致だけ、種類がちがう、ではまとめない。VST3 は Audio Module Class だけを見る。

## 7. 種類の文字列

- LV2 のクラス 38 [S26] A。`Plugin` の直下に Delay、Distortion、Dynamics、Filter、Generator、MIDI、Modulator、Simulator、Spatial、Spectral、Utility。Dynamics の下に Amplifier ・ Compressor ・ Envelope ・ Expander ・ Gate ・ Limiter、Filter の下に Allpass ・ Bandpass ・ Comb ・ **EQ**（その下に MultiEQ ・ ParaEQ）・ Highpass ・ Lowpass、Modulator の下に Chorus ・ Flanger ・ Phaser、Generator の下に Constant ・ **Instrument** ・ Oscillator、Distortion の下に Waveshaper、Spectral の下に **Pitch**、Utility の下に **Analyser** ・ Converter ・ Function ・ Mixer。**Reverb は親が Delay ・ Plugin ・ Simulator の 3 つ**。
- VST3 の `PlugType` [S12] A: `Fx|Analyzer`、`Fx|Delay`、`Fx|Distortion`、`Fx|Dynamics`、`Fx|EQ`、`Fx|Filter`、`Fx|Generator`、`Fx|Mastering`、`Fx|Modulation`、`Fx|Pitch Shift`、`Fx|Restoration`、`Fx|Reverb`、`Fx|Spatial`、`Fx|Surround`、`Fx|Tools` ほか、`Instrument`、`Instrument|Drum`、`Instrument|Piano`、`Instrument|Sampler`、`Instrument|Synth`、`Instrument|Synth|Sampler`、`Spatial`、`Spatial|Fx`、`Analyzer`、`Mono`、`Stereo` ほか。
- CLAP の features [S47] A: 区分 `instrument` ・ `audio-effect` ・ `note-effect` ・ `note-detector` ・ `analyzer`。細かい種類 `synthesizer`、`sampler`、`drum`、`drum-machine`、`filter`、`phaser`、`equalizer`、`de-esser`、`pitch-shifter`、`pitch-correction`、`frequency-shifter`、`distortion`、`transient-shaper`、`compressor`、`expander`、`gate`、`limiter`、`flanger`、`chorus`、`delay`、`reverb`、`tremolo`、`utility`、`restoration`、`mastering`、`mixing` ほか。
- 仕様書 3 章 14-A の表とのちがい: 仕様書の表は `Fx|EQ` などの文字列と CLAP の語でそのまま当てられる。合わない所は、「ピアノ ・ 鍵盤」は VST3 だけ、「サンプラー」「ドラム」は LV2 に語がない、「空間」は CLAP に語がない（`surround` ・ `stereo` ・ `mono` は音の形で種類ではない）、CLAP の `analyzer` は区分の語。LV2 は EQ を Filter より先に、Reverb を Delay より先に見る（仕様書の「上の順でいちばん先に合った所」がそのまま効く順）。
- 実物: Surge XT は LV2 `lv2:InstrumentPlugin` ・ VST3 `Instrument|Synth` ・ CLAP instrument ＋ synthesizer。ZamComp は `lv2:CompressorPlugin` ・ `Fx|Dynamics` ・ audio-effect ＋ compressor [S62] A。

## 8. Linux のふつうの場所（A）

| 形式 | 場所 |
|---|---|
| LV2 | `LV2_PATH` があればそれ、なければ `~/.lv2:/usr/local/lib/lv2:/usr/lib/lv2` [S34〜S36] |
| VST3 | `$HOME/.vst3/`、`/usr/lib64/vst3/`、`/usr/lib/vst3/`、`/usr/local/lib64/vst3/`、`/usr/local/lib/vst3/`、`$APPFOLDER/vst3/`。同じ Processor UID が複数あれば先の物 [S16] |
| CLAP | `~/.clap`、`/usr/lib/clap`、さらに `CLAP_PATH`（コロン区切り）を必ず読む。再帰でさがす [S47] |

## わからなかった事
REAPER ・ Bitwig の監視中の遅れの逃がしかた、Cubase の最新版の文、livi が mapPath をわたさないとき lilv が補うか、Ardour ・ Carla が `patch:Get` をいつ送るか、JUCE の LV2 ラッパーのソース、DPF の VST3 の class ID の組み立ての後半、clack-host のホスト側の拡張の広さ、CLAP 1.2.7 と 1.2.10 の関係。

## 出どころ
| ID | URL または場所 | 確かさ |
|---|---|---|
| S1 | https://crates.io/api/v1/crates/vst3 | A |
| S2 | https://raw.githubusercontent.com/coupler-rs/vst3-rs/master/CHANGELOG.md | A |
| S3 | https://api.github.com/repos/coupler-rs/vst3-rs/commits?per_page=10 | A |
| S4 | `~/.cargo/registry/src/index.crates.io-*/vst3-0.3.0/` | A |
| S5 | https://github.com/steinbergmedia/vst3sdk（LICENSE.txt） | A |
| S6 | https://steinbergmedia.github.io/vst3_dev_portal/pages/VST+3+Licensing/VST3+License | A |
| S7 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Versions/Version+3.8.0.html | A |
| S8 | https://api.github.com/repos/steinbergmedia/vst3_pluginterfaces/commits?per_page=6 | A |
| S9 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Change+History/Index.html | A |
| S10 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/gui/iplugview.h | A |
| S11 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivsteditcontroller.h | A |
| S12 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstaudioprocessor.h | A |
| S13 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/API+Documentation/Index.html | A |
| S14 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/VST+Module+Architecture/ModuleInfo-JSON.html | A |
| S16 | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Plugin+Locations.html | A |
| S17〜S20 | https://crates.io/api/v1/crates/livi 、lilv 、lilv-sys 、lv2_raw | A |
| S22 | https://raw.githubusercontent.com/wmedrano/livi-rs/main/CHANGELOG.md | A |
| S24 | `~/.cargo/registry/src/index.crates.io-*/livi-0.9.0/src/` | A |
| S25 | `~/.cargo/registry/src/index.crates.io-*/lilv-0.3.1/src/` | A |
| S26 | https://lv2plug.in/ns/lv2core | A |
| S27 | https://lv2plug.in/ns/extensions/ui | A |
| S28 | https://lv2plug.in/ns/ext/instance-access | A |
| S29 | https://lv2plug.in/ns/ext/state | A |
| S30 | https://lv2plug.in/ns/ext/patch | A |
| S31 | https://lv2plug.in/ns/ext/parameters | A |
| S32 | https://lv2plug.in/ns/ext/worker | A |
| S33 | https://lv2plug.in/ns/ext/options | A |
| S34 | https://lv2plug.in/pages/filesystem-hierarchy-standard.html | A |
| S35 | https://gitlab.com/lv2/lilv/-/raw/main/meson.build | A |
| S36 | https://gitlab.com/lv2/lilv/-/raw/main/src/world.c | A |
| S37 | https://gitlab.com/drobilla/jalv/-/raw/main/src/jalv.c | A |
| S38 | https://gitlab.com/drobilla/jalv/-/raw/main/src/control.c | A |
| S39 | `/usr/lib/lv2/Surge XT.lv2/`（dsp.ttl、ui.ttl、manifest.ttl） | A（実物） |
| S40 | https://forum.juce.com/t/lv2-plugin-does-not-show-automatable-parameters-in-reaper/68936 | C |
| S41 | https://github.com/surge-synthesizer/surge/pull/1773 | B |
| S42 | https://raw.githubusercontent.com/falkTX/Carla/main/source/backend/plugin/CarlaPluginLV2.cpp | A |
| S44 | https://github.com/prokopyl/clack、https://docs.rs/clack-host/0.2.0/、https://crates.io/api/v1/crates/clack-host | A |
| S45 | https://api.github.com/repos/prokopyl/clack/commits?per_page=8 | A |
| S46 | https://crates.io/api/v1/crates/clap-sys | A |
| S47 | https://github.com/free-audio/clap（include/clap/ のヘッダ） | A |
| S49 | https://www.reaper.fm/whatsnew.txt | A |
| S50 | https://www.kvraudio.com/news/cockos-updates-reaper-to-v6-71---clap-plugin-support-56611 | B |
| S51 | https://surge-synthesizer.github.io/downloads | A |
| S55 | https://archive.steinberg.help/nuendo/v12/en/cubase_nuendo/topics/vst_instruments/vst_instruments_constrain_delay_compensation_c.html（403、検索の要約） | B |
| S56 | https://support.apple.com/105040 | A |
| S57 | https://manual.ardour.org/synchronization/latency-and-latency-compensation/ | A |
| S60 | https://manual.ardour.org/working-with-plugins/plugin-manager/ | A |
| S61 | https://discourse.ardour.org/t/how-does-ardour-determine-what-plugins-are-a-match-and-can-i-manually-enforce-this/111610 | B |
| S62 | `/usr/lib/lv2/`、`/usr/lib/vst3/`、`/usr/lib/clap/` の実物 | A |
| S63 | https://raw.githubusercontent.com/DISTRHO/DPF/main/distrho/DistrhoInfo.hpp | A（一部） |
| S64 | https://raw.githubusercontent.com/juce-framework/JUCE/master/docs/CMake%20API.md | A |
| S66 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/lv2_plugin.cc | A |
