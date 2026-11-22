# 第 4 回: 音色の素材を取ってきて、中身を確かめた（2026-11-18）

meloet-impl（Opus 5.5）が、決まった素材（`docs/research/instrument-samples.md`、`docs/progress.md` 第 4 回の区切り0）を取ってきて、ライセンスの文、形式、SFZ の中身、大きさを確かめた記録です。
取った物は `~/.cache/meloet/sounds-src/<名前>/` に置き、リポジトリには入れていない。取りなおしかたは [`tools/sounds/sources.json`](../../tools/sounds/sources.json)、ライセンスの写しは [`licenses/sounds/`](../../licenses/sounds/)。

確かさ: **A** ＝ 素材の中のファイル（LICENSE、README、SFZ）を読んだ、または ffprobe ・ ffmpeg で測った。**C** ＝ ファイル名や数から読んだ推測。
道具: `ffprobe`・`ffmpeg`（Ubuntu の版）、`python3 -I tools/sounds/probe.py`（形式のまとめ）、`tools/sounds/sfz_flat.py`（SFZ の `#include`・`#define`・見出しの引き継ぎをほどいて 1 リージョン 1 行に）、`tools/sounds/sfz_select.py`（SFZ の条件でファイルを選ぶ）。音の高さは、スクラッチの numpy の FFT（倍音の積）で大まかに測った（オルガンのように倍音の強い音は 1 オクターブまちがえることがある）。

## まとめ

| 素材 | 取った物 | ライセンス（中の文） | 形式 | 使いそうな分 | 気になる所 |
|---|---|---|---|---|---|
| Salamander Grand Piano V3 | もう取ってあった tar.gz（sha256 一致を確かめた） | CC BY 3.0（readme.txt） | — （このたびは見ない） | — | ほかの係が測っている |
| Wurlitzer EP200（Greg Sullivan） | commit 8c3e581 の tar.gz 19.3 MB | リポジトリは CC BY 3.0、作者のサイトは CC BY 4.0 | FLAC 44.1 kHz 16 bit モノ 42 個 | 2.4 MB | **42 個ぜんぶ、音の途中で切れている**（ループが失われている） |
| Naked Drums | commit 407732a、SFZ 一式 ＋ 11 の音（タム 5 つ、クローズ 2 種）の全マイク 2400 個 | CC BY 4.0（README、LICENSE） | FLAC 44.1 kHz 24 bit、直のマイクはモノ、ほかは主にステレオ | 全マイク 654 MB、直＋オーバーヘッドだけなら 224 MB（タム 3 つ・クローズ 1 種なら 197 MB） | ステレオにまとめたファイルはない（鳴らすときにまぜる）。ペダル HH の直のマイクは無音 |
| VCSL Claps | commit c1ea7bc の Claps フォルダ 10 個 | CC0（LICENSE、README） | WAV、2 種の形式がまざる | 1.3 MB | どちらが何の録音かの説明なし |
| VCSL Pipe Organ | 同じ commit の Pipe Organ フォルダ 65 個 ＋ Info.txt | CC0 ＋ Info.txt の再配布の許し | WAV 44.1 kHz 16 bit ステレオ | 手鍵盤 90 MB（足鍵盤も入れると 138 MB） | Loud と Quiet は同じ音の強弱ではなく、別の音栓 ・ 別のマイク（ファイル名から、C） |
| Black And Blue Basses | tag v1.002（commit 6e7d674）の黒の指弾きとリリース 840 個 ＋ SFZ 一式 | CC0（license） | WAV 44.1 kHz 24 bit モノ | 471 MB | 鍵の番号より 1 オクターブ低く鳴る。どのファイルも 5.00 秒で、鳴っている途中で切れる |
| VSCO 2 CE | commit 4403009 のセクション 3 つ ＋ コントラバスののばす音 129 個、SFZ は SFZ ブランチ（6dd651d）から読んだ | CC0（LICENSE）＋ Readme.txt の「売らないで」のお願い | WAV 44.1 kHz、16 bit と 24 bit がまざる、ステレオ | 238 MB（ノンビブラートのコントラバスも入れると 291 MB） | ループなし（6〜17 秒） |

ディスク: `~/.cache/meloet/sounds-src/` の下に新しく 2.9 GB（うち `.git` が 1.37 GB。git の部分取りは、取り出したファイルと同じ中身を `.git` にも持つ）。Salamander の 2 つのディレクトリにはさわっていない（readme.txt だけを tar から `salamander-meta/` に出した）。

## ライセンス

| 素材 | 中の文 | 外の文 | 食いちがい・条件 | 確かさ |
|---|---|---|---|---|
| Salamander | FreePats の readme.txt: 「Creative Commons Attribution 3.0」、作者 Alexander Holm。作者の README の写しも中にある（作者のメールアドレスつき） | FreePats のページも CC BY 3.0 | なし。帰属表示に要る物は sources.json | A |
| Wurlitzer EP200 | README.md と LICENSE: CC BY 3.0 Unported。「with the author permission with the request for attribution」 | 作者のサイト（2026-11-18 10:45 UTC に取った）: 「Licensing (updated 21-Oct-2026) : All these instruments I host here now have Creative Commons Attribution 4.0 International licensing. (CC BY 4.0)」。写しは `licenses/sounds/gregsullivan-epianos/sullivang.net-2026-11-18.txt` | **食いちがいあり**（3.0 と 4.0）。どちらも CC BY。kinwie の SFZ の割りあて（鍵の範囲、tune、volume）は、リポジトリの CC BY 3.0 だけ | A |
| Naked Drums | README.md: CC BY 4.0 International。LICENSE は CC BY 4.0 の全文。SFZ の頭に「© Wilkinson Audio 2016」 | — | なし | A |
| VCSL | LICENSE: CC0 1.0。README:「no royalties, no credit, no special terms」。Pipe Organ の Info.txt:「Organ sampled by Simon Dalzell of Ivy Audio, Winter 2015-16. Bearer is granted right to redistribute this sample set by Versilian Studios LLC. Credit to both original author and Versilian Studios is encouraged.」 | — | オルガンは Versilian 以外の人（Simon Dalzell）の録音で、再配布の許しが Info.txt にある。名前の表示はお願い（条件ではない） | A |
| Black And Blue Basses | license: CC0 1.0 の全文。使いかたの PDF:「This is free and open-source, with a CC 0 license.」「Recorded, edited, programmed and even photographed by Drogomir Smolken.」 | — | なし（調べ物にあった、昔の記事の CC BY 3.0 とは食いちがう。中の文を採る） | A |
| VSCO 2 CE | LICENSE: CC0 1.0。Readme.txt（2016）:「You are permitted to use these samples for ANY purpose. We ask that you do not sell the samples directly, and encourage you to keep any work done on improving the sample set open and public. Please provide credit to Versilian Studios/Sam Gossner, and/or Ivy Audio/Simon Dalzell where applicable, and link to the VSCO: CE homepage.」 | — | **CC0 のほかに「素材をそのまま売らないで」のお願い**がある。CC0 の文が条件で、こちらはお願い（ask）。meloet は無料で配り、素材だけを売るわけでもないので、ぶつからないと読む（C）。名前とページへのリンクを出すのがよい | A（文）・C（読み） |

PDF の使いかたの書類（Black And Blue Basses、6.7 MB）は写していない。必要な文は上と sources.json に引いた。

## Wurlitzer EP200（Greg Sullivan、SFZ は kinwie）

| 項目 | 結果 | 確かさ |
|---|---|---|
| ファイル | `Wurlitzer EP200/Samples/*.flac` 42 個、合わせて 2.4 MB | A |
| 形式 | 42 個ぜんぶ FLAC 44.1 kHz 16 bit モノ | A |
| 長さ | 0.36〜5.58 秒（いちばん短い db5pp 0.36 秒、g5mp 0.66 秒、b4mp 0.78 秒。長いのは低い音 a1ff 5.58 秒） | A |
| 鍵の範囲 | 33〜96（A1〜C7、64 鍵）。リージョンは 1 つが 2〜9 鍵 | A |
| もとの鍵（pitch_keycenter） | pp: 33, 44, 51, 56, 61, 67, 73, 79, 85, 92（11 リージョン、33 は 2 つ）／ mp: 33, 40, 45, 51, 56, 61, 66, 71, 73, 79, 85, 92／ f: 33, 36, 41, 47, 52, 56, 61, 68, 73, 79, 85, 92／ ff: 33, 36, 41, 47, 52, 56, 61, 64, 67, 73, 79, 85, 92。どれも tune（−12〜+7 セント）で直している | A |
| ベロシティの段 | 4 段: pp 1–37、mp 38–65、f 66–89、ff 90–127。上のほうでは段が別の段のファイルを使う（pp の 79 ・ 85 ・ 92 は mp のファイル、ff の 85 ・ 92 は f のファイル） | A |
| ラウンドロビン | なし | A |
| リリース音 | なし（`ampeg_release` は CC72 で 0〜2 秒、はじめは 0.05 × 2 = 0.1 秒） | A |
| エンベロープ | attack 0.001、hold 5 秒、decay 25 秒、sustain 0。`note_polyphony=1`（同じ鍵を弾きなおすと前の音を止める、off_time 0.5） | A |
| ループ | SFZ にループの指定がない。FLAC にも smpl などのループの情報がない | A |
| **音の終わり** | **42 個ぜんぶ、最後の 20 ms がまだ大きい（最大値 −4.8〜−27.0 dBFS）**。音が消える前にファイルが終わっている。作者のページには「Clean up the audible loop artifacts」とあり、もとの Gigastudio の楽器（wurlitzer_ep200.rar、作者のサイト）はループで音をのばしていたと読める。SFZ への変換でループが落ち、短い音のままになった（C） | A（測定）・C（理由） |
| 楽器の名前 | リポジトリは「Wurlitzer EP200 Electric Piano v1.1 (16-May-1999)」。作者のページは「Wurlitzer EP203W Electric Piano」「my own Wurlitzer EP203W (a member of the EP200 series)」 | A |

## Naked Drums（Wilkinson Audio、SFZ は kinwie）

### 作りとステレオ

- **「stereo-out」はファイルではなく、鳴らしかた**。`Naked Drums (stereo-out).ariax` と `Stereo/Naked Drums.sfz` は、16 のマイクのファイルを、鳴らすときに 1 つのステレオにまぜる。2 チャンネルにまとめたファイルは入っていない。[A]
- マイクの層（`Data/macro.txt` の CC。リージョンは `locc41`〜`locc46` のどれかを持つ）:

| 層 | CC | 中身 | チャンネル | はじめの量（Stereo/Naked Drums.sfz） | 確かさ |
|---|---|---|---|---|---|
| 直（Direct） | 41 | キック In ・ Sub、スネア Top1 ・ Top2 ・ Bottom、タム 1〜5、ハイハット、ライド | モノ（CC でパン: タム 1〜5 が 0.2 ・ 0.35 ・ 0.5 ・ 0.65 ・ 0.8、HH 0.35、ライド 0.65。左が高いタムで、叩く人から見た並び） | 100 %（HH とライドの直だけ 20 %） | A |
| オーバーヘッド（OH） | 42 | 全部（シンバルも） | ステレオ | 100 % | A |
| 近い部屋（CR） | 43 | 全部 | **モノ** | 100 % | A |
| 遠い部屋（FR） | 44 | 全部 | ステレオ | 100 % | A |
| ミッドサイド（MS） | 45 | 全部 | ステレオ（L と R の大きさが同じで、L−R が −48 dB と小さいので、もう L ・ R にほどいてあると読む） | 100 % | A（チャンネル）・C（ほどいてある） |
| China Wide OH（CH） | 46 | ハイハット、ライド、シンバル | ステレオ | 20 % | A |

- 作者の GM 用の組み（`User/Naked Drums GM.sfz`）は、直と OH の 2 層だけを使う。[A]
- 層ごとの音量のなおし（`macro.txt` の TRIM）: OH のキック +6 dB、部屋の Tom5 +7 dB、ライド −3〜−6 dB、シンバル −3 dB（スプラッシュ −9 dB）など。[A]
- 全部 one shot（`loop_mode=one_shot`）、`seq_length=10`（10 テイクを順に）。同じ鍵の同時発音: 太鼓 1、HH 3、シンバル 4。[A]

### 12 の音

ファイル名（`-OH_Black_D2_50_one-QGEQ.flac` など）は、もとの Kontakt のフォルダの名前をつかいまわしていて、どの太鼓かを表さない（キックの直のマイクが `-Bottom_Black_D2_...`、スネアの裏が `Bottom_Black_D2_...`）。どのファイルがどの音かは SFZ（`Data/group/*.txt` → `Data/file/<4 文字>.txt`）からしかわからない。`tools/sounds/sfz_select.py` で選ぶ。[A]

| meloet の音（GM の鍵） | Naked Drums の音（master_label / group_label） | SFZ の鍵（はじめの keymap ／ GM の keymap） | ベロシティの段 | テイク | マイク | 長さ | 確かさ |
|---|---|---|---|---|---|---|---|
| キック（36） | Kick / Center | 36 ／ 36 | 5 段: 1–27、28–52、53–77、78–102、103–127 | 10 | 直 2 本（In、Sub）、OH、CR、FR、MS | 2.00〜3.00 秒 | A |
| サイドスティック（37） | Snare1 / Sidestick | 37 ／ 37 | 1 段 | 10 | 直 3 本（Top1、Top2、Bottom）、OH、CR、FR、MS | 1.41 秒 | A |
| スネア（38） | Snare1 / Center | 38 ／ 38 | 5 段（キックと同じ境） | 10 | 直 3 本、OH、CR、FR、MS | 1.50〜3.00 秒 | A |
| クローズ HH（42） | 案 A: Hats / Tight（GM の keymap で 42）、案 B: Hats / Closed（はじめの keymap で 42、GM では 54） | 39 ・ 42 ／ 42 ・ 54 | 3 段: 1–43、44–85、86–127 | 10 | 直、OH、CR、FR、MS、CH | Tight 1.00 秒、Closed 1.18〜1.38 秒 | A |
| ペダル HH（44） | Hats / Pedal | 44 ／ 44 | 1 段 | 10 | **直のマイクの 10 個は無音**（184 バイト、−91 dB）。OH と部屋だけで鳴る | 0.69 秒 | A |
| オープン HH（46） | Hats / Open | 49 ／ 46 | 3 段 | 10 | 直、OH、CR、FR、MS、CH | 4.01〜4.62 秒 | A |
| ハイタム（50） | 案: Tom1（10″ と読む） | 48 ／ 48 | 3 段 | 10 | 直、OH、CR、FR、MS | 2.00〜2.29 秒 | A（SFZ）・C（大きさ） |
| ミッドタム（47） | 案: Tom3 | 45 ／ 45 | 3 段 | 10 | 同じ | 2.00〜2.60 秒 | 同じ |
| ロータム（45） | 案: Tom5（16″ のフロアタムと読む） | 41 ／ 41 | 3 段 | 10 | 同じ | 2.00〜2.60 秒 | 同じ |
| クラッシュ（49） | Crash1 / Edge | 55 ／ 49 | 3 段 | 10 | **直のマイクなし**。OH、CR、FR、MS、CH | 5.00〜6.30 秒 | A |
| ライド（51） | Ride / Bow | 51 ／ 51 | 3 段 | 10 | 直、OH、CR、FR、MS、CH | 7.79〜12.13 秒 | A |
| クラップ（39） | VCSL（下） | — | — | — | — | — | — |

- **同時に鳴らさない組**: クローズ（Tight、Closed）とペダルを叩くと、無音のリージョン（`*silence`、group 200〜203）が鳴り、オープン（`off_by=203`）とハーフ（`off_by=202`）が止まる。GM の 42 ・ 44 が 46 を止めるのと同じ。シンバルは、別の鍵（60 以上）かポリフォニックアフタータッチで止める（チョーク）。[A]
- 全部 FLAC 44.1 kHz 24 bit。ファイルの終わり（最後 50 ms、いちばん長いテイク）は、どれも −51 dBFS より小さい（多くは −90 dB）。[A]

### タムの割りあて（鍵 50 ・ 47 ・ 45）

直のマイク（Top）のいちばん強い段の 1 テイク目で、0.05〜1.0 秒の主な高さを測った。[A（測定、ただしタムの高さは叩いてすぐ下がるので目安）]

| Naked Drums | 測った高さ | SFZ の tune | 鳴る高さ（tune 込み） | README の太鼓（10″、12″、13″、14″、16″） |
|---|---|---|---|---|
| Tom1 | 126 Hz | 0 | 126 Hz | 番号の順に 10″ と読む（C） |
| Tom2 | 99 Hz | 0 | 99 Hz | 12″（C） |
| Tom3 | 91 Hz | −200 セント | 約 81 Hz | 13″（C） |
| Tom4 | 75 Hz | −70 セント | 約 72 Hz | 14″（C） |
| Tom5 | 46 Hz | 0 | 46 Hz | 16″（C） |

- 番号が大きいほど低い（Tom1 → Tom5）。[A]
- 案: **ハイ 50 ＝ Tom1、ミッド 47 ＝ Tom3、ロー 45 ＝ Tom5**（いちばん広く分かれる）。もう 1 つの案: Tom1 ・ Tom2 ・ Tom4（ラックタム 2 つ ＋ 14″）。どちらも聞いて決める。5 つとも全マイクを取ってあるので、取りなおさずに選べる。

### 大きさ

| 範囲 | ファイル | FLAC の大きさ |
|---|---|---|
| 上の 11 の音 ＋ タム 5 つ ＋ クローズ 2 種、全マイク | 2400 | 654 MB |
| 同じ、直 ＋ OH だけ（作者の GM の組み） | 980 | 224 MB |
| 直 ＋ OH、タム 3 つ（Tom1 ・ 3 ・ 5）、クローズは Tight だけ | 800 | 197 MB |
| リポジトリ全体（調べ物の数） | 3840 | 約 1.3 GB |

## VCSL Claps

| ファイル | 形式 | 長さ | 最大の大きさ | 確かさ |
|---|---|---|---|---|
| `Clap_rr1.wav`〜`Clap_rr6.wav` | WAV 48 kHz 16 bit ステレオ | 0.47〜0.77 秒 | どれも −0.4 dBFS（そろえてある） | A |
| `SoloClap_vl1.wav`〜`SoloClap_vl4.wav` | WAV 44.1 kHz 24 bit ステレオ | 0.31〜0.63 秒 | −14.8、−8.8、−4.5、−0.7 dBFS（4 段の強さ） | A |

- **2 種の形式がまざっている**（48 kHz/16 bit と 44.1 kHz/24 bit）。[A]
- 「Clap」が何人の手拍子か、「SoloClap」が 1 人か、の説明はフォルダにない（Info.txt なし）。名前からは、Clap ＝ 6 テイク（強さ 1 つ）、SoloClap ＝ 4 段の強さ（テイク 1 つ）と読む。[C]
- リポジトリに SFZ はない（README:「SFZ version ... from the latest release」。release は取っていない）。

## VCSL Pipe Organ

| フォルダ | 中身 | 鍵（ファイル名） | 形式 | 長さ | 確かさ |
|---|---|---|---|---|---|
| `Loud/` | `Rode_Man3Open_*.wav` 21 個 | C1〜C6、短 3 度ごと（C、D#、F#、A） | WAV 44.1 kHz 16 bit ステレオ | 11.90〜12.66 秒 | A |
| `Quiet/` | `NT5_Man3Quiet_*_rr1.wav` 21 個 | 同じ | 同じ | 11.43〜12.98 秒 | A |
| `Loud Pedal/` | `Rode_Pedal_*.wav` 11 個 | C1〜F#3、短 3 度ごと | 同じ | 12.44〜13.77 秒 | A |
| `Quiet Pedal/` | `NT5_PedalQuiet_*_rr1.wav` 11 個 | 同じ | 同じ | 11.68〜12.26 秒 | A |
| `Rode_Blower.wav` | 送風機の音 | — | 同じ | 37.17 秒 | A |

- **名前の C3 が MIDI 60**: Quiet の C1 65 Hz、C2 131 Hz、C3 262 Hz、C5 1045 Hz、A3 439 Hz と測れた（Loud は音栓が多く、倍音で高さがぶれる）。手鍵盤は鍵 36〜96、足鍵盤は 36〜66。[A（Quiet の測定）・C（Loud も同じと読む）]
- **Loud と Quiet は別の音栓 ・ 別のマイク**（Rode と NT5、「Man3Open」と「Man3Quiet」）。同じ音の強さちがいではない。ベロシティで切りかえるかは聞いて決める。[C（ファイル名から）]
- ループなし（WAV に smpl なし）。約 10 秒は同じ大きさでのび（C3: 2 秒 −27.8 dB、9 秒 −30.7 dB）、11 秒ごろに鍵を離した音と響きになり、終わりは無音（−87〜−91 dB）。10 秒より長くのばすには、こちらでループを作る。[A]
- テイクは 1 つ（rr1 だけ）。[A]

## Black And Blue Basses（Karoryfer）

| 項目 | 結果 | 確かさ |
|---|---|---|
| 指弾きのファイル | `Samples/darkblack/reg/`（672 個）＝ 黒のベース（Washburn AB-95）を指で弾き、ネックのピックアップで録った音（使いかたの PDF）。SFZ は `Programs/05-darkblack_pluck.sfz`（`01-darkblack_keysw.sfz` の Pluck と同じ地図） | A |
| リリース音 | `Samples/darkblack/rel/`（168 個 ＝ 42 鍵 × 4 テイク、0.70〜1.69 秒、`trigger=release`）。CC107 が 0 のときは鳴らない。SFZ のはじめの値は CC107 を決めていないので、**はじめは鳴らない** | A |
| 形式 | 840 個ぜんぶ WAV 44.1 kHz 24 bit モノ | A |
| 鍵の範囲（SFZ） | 35〜76（42 鍵、1 鍵 1 ファイル。5 弦 BEADG、21 フレット） | A |
| **鳴る高さ** | **鍵より 1 オクターブ低い**。`darkblack_b1_*` は鍵 35 に置かれ、測ると 30.9 Hz（MIDI 23 の B0）。e2 は 41.2 Hz（28）、e4 は 164 Hz（52）。ベースの楽譜と同じ書きかた。鳴る範囲は 23〜64 | A |
| ベロシティの段 | 4 段: 1–31（p）、32–63（mp）、64–95（mf）、96–127（f）。段の中は `amp_velcurve` で音量を変える | A |
| テイク | 4（`seq_length=4`） | A |
| 長さ | 672 個ぜんぶ 5.00 秒ちょうど。**鳴っている途中で切れている**（b1 f の最後 50 ms が −46.8 dBFS、e4 f が −65.7 dBFS）。SFZ は `ampeg_release=0.25` 秒 | A |
| ループ | なし | A |
| 鳴らしかた | はじめはポリ（CC105 が 0）。SFZ にはローパスのフィルター（cutoff 250 Hz × CC90 などで 9000 Hz まで、キーで追う）、ビブラート、ユニゾンが入っている。元の音に近づけるにはフィルターの値も見る。`c4` の 20 リージョンに pitch_keycenter がないが、SFZ のはじめの値が 60 なので合っている | A |
| 使わない物 | ゴースト（336）、スタッカート（168）、ブリッジの後ろ（168 ＋ 36）、青のベース（ピック、660） | A |

## VSCO 2 CE のストリングス

リポジトリの master には SFZ がない。README のとおり **SFZ ブランチ**（commit 6dd651d、同じサンプルの道）に SFZ がある。そこから 5 つを `git show` で読んだ（サンプルは取りなおしていない）。

| 楽器（SFZ） | ファイル | 鍵の範囲（もとの鍵） | ベロシティの段 | 形式 | 長さ | 大きさ | 確かさ |
|---|---|---|---|---|---|---|---|
| ヴァイオリン ・ セクション（`ViolinEnsSusVib.sfz`） | `Strings/Violin Section/susVib/` 22 個 | 55〜86（11 音: 55, 57, 59, 62, 66, 69, 72, 76, 79, 83, 86） | 2 段: v1 ＝ 0–62（volume +20 dB）、v2 ＝ 63–127（+7 dB） | WAV 44.1 kHz **16 bit** ステレオ | 8.99〜15.20 秒 | 46.0 MB | A |
| ヴィオラ ・ セクション（`ViolaEnsSusVib.sfz`） | `Strings/Viola Section/susvib/` 26 個 | 48〜86（13 音） | 2 段: v1 0–62（+28 dB）、v2 63–127（+7 dB） | WAV 44.1 kHz **24 bit** ステレオ | 7.56〜13.75 秒 | 71.7 MB | A |
| チェロ ・ セクション（`CelloEnsSusVib.sfz`） | `Strings/Cello Section/susvib/` 27 個 | 36〜77（13 音） | 2 段: v1 0–62（+16 dB）、v3 63–127（+4 dB）。E1（40）だけ v1 を 0–41 ＝ `_1`、42–62 ＝ `_2` に分ける | WAV 44.1 kHz 24 bit ステレオ | 6.39〜12.75 秒 | 72.6 MB | A |
| コントラバス ・ ソロ、ビブラート（`ContrabassSusVB.sfz`） | `Strings/Solo Contrabass/SusVib/` 26 個 | 24〜60（13 音、いちばん下の F#0 を 24〜30 にのばす） | 2 段: v1 0–62（+18 dB）、v3 63–127（+6 dB） | WAV 44.1 kHz 16 bit ステレオ | 6.54〜17.33 秒 | 48.0 MB | A |
| コントラバス ・ ソロ、ノンビブラート（`ContrabassSusNV.sfz`） | `Strings/Solo Contrabass/SusNV/` 28 個 | 24〜60（14 音、E0 あり） | 2 段: v1 0–62（+20 dB）、v3 63–127（+5 dB） | WAV 44.1 kHz 16 bit ステレオ | 6.59〜18.20 秒 | 52.8 MB | A |

- **v1 ・ v2（v3）はベロシティの段**で、テイクではない。v1 は小さく録られていて、SFZ が +16〜+28 dB 上げている。チェロの v1 だけ、E1 に 2 テイク目（`_2`）があり、ベロシティで分けている。[A]
- ファイル名の C3 が MIDI 60（SFZ の pitch_keycenter と、ヴァイオリン G2 195 Hz ＝ 55、チェロ C1 の 3 倍音 196 Hz ＝ 65.4 Hz × 3、コントラバス E0 の倍音 82.4 ・ 123.6 Hz ＝ 41.2 Hz × 2 ・ 3 で確かめた）。[A]
- ループなし（WAV に smpl なし、SFZ に loop の指定なし）。終わりは弓を止めて消えている（最後 50 ms が −80〜−91 dBFS）。のばせるのは 6.4〜18 秒。[A]
- SFZ の `ampeg_release` は 0.8〜1 秒、`ampeg_dynamic=1`。[A]
- 形式がまざる: ヴァイオリンとコントラバスは 16 bit、ヴィオラとチェロは 24 bit。[A]

## わからなかった所

- Wurlitzer のループ点: もとの Gigastudio の .gig（作者のサイトの `wurlitzer_ep200.rar`、3 MB）にはありそうだが、決まった素材の外なので取っていない。
- VCSL の SFZ（release の download）を取っていないので、クラップとオルガンの、作者側の鍵 ・ ベロシティの割りあては見ていない（ファイル名と測った高さから読んだ）。
- クラップの Clap と SoloClap の録りかたのちがい。
- Naked Drums のタムの大きさ（10″〜16″）と Tom1〜5 の対応（番号と高さの順が合うので、そう読んだ）。MS の層がもう L ・ R にほどいてあるか。
- 音の質（古い Wurlitzer、教会のオルガン、ジャズ寄りかどうか）は聞いていない。
