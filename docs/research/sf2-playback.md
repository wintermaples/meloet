# SF2 をアプリの中で読んで鳴らす

調べた日 2026-11-19（meloet-research、Sonnet 5.5）。確かさは A（一次情報）・B（一次情報を引いた物）・C（推測）。FluidR3 のどのゾーンがフィルターやモジュレーターを使っているかは、調べ物の係が数えられなかった（shell がない）。下の「FluidR3 で数えた事」は棟梁が `tools/sounds/sf2.py` で数えた。

## 結論

1. **Rust の部品**（A）
   - `rustysynth` 1.3.8（MIT、2026-10-20、依存なし）: 1 本の完成したシンセ。モジュレーターを捨て、sm24 を読まず、SF3 を断り、減衰が振幅の指数で幅 80 dB（仕様は 96 か 100 dB）。ファイルまるごとをメモリに持つ。音のスレッドでの確保はない（エフェクトの中は未確認）。**丸ごと入れるのはすすめない。鳴らしかたの参考に読む**（MIT なので帰属表示をつければ写せる）。
   - `oxisynth` 0.1.0、`oxisynth-chorus` ・ `-reverb`: **LGPL-2.1。入れられない**。`xsynth-*` 0.4.0: LGPL-3.0。`fluidlite`: LGPL-2.1。
   - `soundfont` 0.1.0（MIT、依存なし、1 回だけの公開）: 読むだけの部品。モジュレーターを最後まで解き、サンプルは位置だけ持ち、sm24 の位置も持つ。グローバルゾーンの区別と足しあわせはしない。**予備の案**。
   - 提案: **パーサは自分で書く（`sf2.py` を Rust に写す）。鳴らす所は、いまのサンプルを鳴らす係（`meloet-sound/src/sampler/`）を広げる**。SF2 は読みこみのときに `Region` に変えて、音のスレッドは SF2 を知らない。
2. **仕様 2.01 のうち要る物**は下の段の案。仕様の本文（SoundFont 2.01 Technical Specification、1998）で確かめた（A）。
3. **FluidSynth とのちがい**（現行のソースを読んだ。LGPL なので写さない。A）
   - `initialAttenuation` の 0.4 倍は、いまも読みこみのとき、プリセットとインストゥルメントのゾーンの値だけにかける（モジュレーターの 960 cB にはかけない）。
   - 強さ → フィルターのデフォルトのモジュレーター（仕様 8.4.2、−2400 cent）は、実際には働かせていない（wiki も「規格の中で食いちがうので外した」）。rustysynth も入れていない。
   - 音量の減衰 ・ リリースは cB でまっすぐ、幅 96 dB。
   - フィルターは RBJ の 2 次の LPF。Q は cB から 3.01 dB 引いて（Q 0 で山なし）、利得を 1/√Q で補正。カットオフは 5 Hz〜0.45×サンプルレート。
   - pan のデフォルトのモジュレーターは 500（仕様は 1000）。
   - 提案（C）: **FluidSynth に合わせる**（0.4 倍は定数 1 つで切りかえられる形に、強さ → フィルターは入れない、96 dB）。FluidR3 は Linux ではふつう FluidSynth で鳴らされるため。ファイルの中の明示的なモジュレーター（IMOD ・ PMOD）は第 3 の段で仕様どおりに。
4. **SF3**（Ogg Vorbis でちぢめた形。MuseScore が作った公式でない拡張）: 出回りの数は見つからない。**最初は「読めません」と言う**でよい。見わけは `ifil` の major が 3、`shdr` の 0x10 のビット、`smpl` の中の "OggS"（A: FluidSynth の wiki）。
5. **GM**: 16 の仲間は 8 個ずつ（B: 一次の RP-003 は会員向け）。ドラムは慣例でバンク 128（A: 仕様 7.2）。GS ・ XG の別の音色は、`wBank` が 0 でない通常のプリセット（A）。

## 段の案

### 第 1 の段「鳴る」
1. パーサ: 足しあわせ（インストゥルメントのゾーンは絶対で置きかえ、プリセットのゾーンは相対で足す、keyRange ・ velRange は重なり）、グローバルゾーン、プリセットでは無視する物（0・1・2・3・4・12・45・46・47・50・54・57・58。`sf2.py` の `NOT_IN_PRESET` と同じ）、ステレオの組（`sfSampleType` 右 2 ・ 左 4、`wSampleLink`）、`originalPitch` 255 は 60。
2. バンクつきのプリセットの一覧（ドラムはバンク 128）。同じ `(wBank, wPreset)` が 2 つなら先の物。
3. ゾーン → `Region`（読みこみのとき 1 回、音のスレッドの外で）: 鍵と強さの範囲、ルート、coarseTune ・ fineTune ・ scaleTuning ・ pitchCorrection、ループ（sampleModes 0 ・ 1 ・ 3）、位置のずらし、pan、exclusiveClass（範囲はプリセットまるごと）、initialAttenuation。
4. **音量のエンベロープ 6 段**（delay ・ attack ・ hold ・ decay ・ sustain ・ release）。attack は振幅でまっすぐ、decay と release は dB でまっすぐ。keynumToVolEnvHold ・ Decay（キー 60 が基準）。**いまの鳴らす係に足りないのは decay → sustain、hold、delay**。これがないと、ピアノ、ギター、ハープ、ピチカートのような自然に減る音が、ループがあると鳴りつづける（C）。
5. デフォルトのモジュレーターのうち、強さ → 音量（`(v/127)²`、いまの `player.rs` と同じ形）、CC7 ・ CC11（音量）、CC10（pan）、ピッチベンド。

### 第 2 の段「ほぼ同じに聞こえる」
フィルター（2 次の LPF。既定の initialFilterFc 13500 cent は素通しなので、使うゾーンだけ効く）、2 つの LFO（三角波）、モジュレーションのエンベロープ（音の高さとカットオフ）、CC1 とチャンネル圧 → ビブラート、chorus ・ reverb の送り（meloet のエフェクトへ）。

### 第 3 の段「全部」
一般のモジュレーター、keynum ・ velocity のジェネレーター、右のサンプルで音の高さを決める、sm24、補間の質、SF3、声の取り合いの重み。

### 段をまたぐ注意
- メモリには、えらんだプリセットが使うサンプルだけを読む。SF2 のプリセット 1 つを 1 つの `SampleBank` にするのが素直（C）。
- 聞きくらべは、この PC の `/usr/bin/fluidsynth`（`libfluidsynth.so.3.3.8`）を別のプロセスで動かし、同じ MIDI を `-F` でファイルに書いてくらべる（リンクも同梱もしないのでライセンスの問題はない）。
- この PC の `/usr/share/sounds/sf2/` には `TimGM6mb.sf2`（GPL-2。試験だけ、同梱しない）などもある。

## FluidR3 で数えた事（棟梁、2026-11-19、`tools/sounds/sf2.py` の `regions()` を全プリセットに回した。A: 自分の数え）

- プリセット 189（バンク 0 に 128、8 に 28、9 に 1、16 に 1、128（ドラム）に 31）。プリセット × インストゥルメントのゾーン 16912。`ifil` 2.1。
- その音色のゾーンのどれかが使っている物（プリセットの数）: 送り（chorus ・ reverb）178、ずっとループ（sampleModes 1）168、**initialFilterFc が 13500 未満 125**、sustainVolEnv が 0 より大きい 125、hold 104、**modEnvToFilterFc 84**、ループなし 61、initialFilterQ 52、exclusiveClass 31、modLfoToVolume 29、modLfoToPitch 11、modLfoToFilterFc 10、modEnvToPitch 9、delayVolEnv 8、vibLfoToPitch 7、keynumToVolEnv 1。
- モジュレーター: インストゥルメント側 747 のうち 738 が「強さ → initialFilterFc（量の元は強さのスイッチ）」で、デフォルトの 8.4.2 と同じ形（FluidSynth はこの形を 0 にする）。ほかは 4 ・ 4 ・ 1。プリセット側は 1。
- **フィルターは、プリセットの 3 分の 2 が使う**ので、第 2 の段のフィルターとモジュレーションのエンベロープ → フィルターは、「FluidR3 がちゃんと鳴る」に要る。LFO は少ない。一般のモジュレーターは、FluidR3 では強さ → フィルターだけなので、第 3 の段でよい。

## GM の 16 の仲間（B: Wikipedia が一次を引いた物）

| 仲間 | GM の番号（1 から） | SF2 の `wPreset`（0 から） |
|---|---|---|
| ピアノ | 1–8 | 0–7 |
| クロマチック ・ パーカッション（仕様書の「マレット」） | 9–16 | 8–15 |
| オルガン | 17–24 | 16–23 |
| ギター | 25–32 | 24–31 |
| ベース | 33–40 | 32–39 |
| ストリングス（仕様書の「弦」） | 41–48 | 40–47 |
| アンサンブル | 49–56 | 48–55 |
| ブラス（金管） | 57–64 | 56–63 |
| リード（木管） | 65–72 | 64–71 |
| パイプ（笛） | 73–80 | 72–79 |
| シンセ ・ リード | 81–88 | 80–87 |
| シンセ ・ パッド | 89–96 | 88–95 |
| シンセ ・ エフェクト | 97–104 | 96–103 |
| エスニック（民族楽器） | 105–112 | 104–111 |
| パーカッシブ（打楽器） | 113–120 | 112–119 |
| 効果音 | 121–128 | 120–127 |

## 出どころ
| URL | 確かさ |
|---|---|
| https://crates.io/api/v1/crates/rustysynth（ほか oxisynth、soundfont、xsynth-soundfonts、xsynth-core、fluidlite、tsf-sys） | A（2021 年の物は B） |
| https://raw.githubusercontent.com/sinshu/rustysynth/master/（README、Cargo.toml、rustysynth/src/ の voice.rs ・ volume_envelope.rs ・ bi_quad_filter.rs ほか） | A |
| https://raw.githubusercontent.com/PolyMeilex/OxiSynth/master/（Cargo.toml、soundfont-rs/） | A |
| https://docs.rs/soundfont/0.1.0/soundfont/、https://docs.rs/rustysynth/1.3.8/rustysynth/ | A |
| http://www.synthfont.com/SFSPEC21.PDF（SoundFont 2.01 Technical Specification。7.2〜7.10、8.1〜8.5、9.1〜9.5） | A |
| https://raw.githubusercontent.com/FluidSynth/fluidsynth/master/src/sfloader/fluid_defsfont.c、src/synth/fluid_voice.c、src/rvoice/fluid_rvoice.c、src/rvoice/fluid_iir_filter*.c*、src/rvoice/fluid_lfo.h、src/synth/fluid_mod.c、src/synth/fluid_synth.c | A（LGPL。読むだけ） |
| https://www.fluidsynth.org/wiki/SoundFont/ | A |
| https://www.fluidsynth.org/api/settings_synth.html | A |
| https://www.fluidsynth.org/wiki/SoundFont3Format/ | A |
| https://musescore.org/en/handbook/3/soundfonts-and-sfz-files | A |
| https://polyphone.io/en/documentation/manual/annexes/the-different-soundfont-formats | A（「10 倍」は B） |
| https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/main/README.md | A |
| https://midi.org/general-midi-level-1 | A（本文は会員向けで読めない） |
| https://en.wikipedia.org/wiki/General_MIDI | B |
| https://lists.nongnu.org/archive/html/fluid-dev/2010-01/msg00024.html | C（2010 年） |

## わからなかった事
SF3 の出回りの数、SF2 2.04 の本文（sm24、8.4.2 の変更）、GM1 の一次の本文、rustysynth のエフェクトの中の確保。
