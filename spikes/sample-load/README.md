# sample-load — サンプルのピアノは、どれだけ大きく、どれだけ重いか

第 4 回の区切り0 の試作です。Salamander Grand Piano V3（CC BY 3.0）を、meloet の「サンプルを鳴らす係」で使うときに、
メモリに全部のせるか、ディスクから少しずつ読むかを決めるための数を出します。音は鳴らしません。
結果は [`docs/measurements/2026-11-18-round4-sample-load.md`](../../docs/measurements/2026-11-18-round4-sample-load.md)。

本番のコードではありません。ここから写さず、学んだことだけ使ってください。

## 何を測るか

| コマンド | 測ること |
|---|---|
| `sfz` | SFZ を読み、種類（メインの音・リリース音・共鳴・ペダル）ごとの数、鍵とベロシティの範囲を表にする |
| `scan` | 全サンプルを symphonia で読み、長さ、ピーク、頭の無音、ピークから −20〜−60 dB と −72 dBFS まで下がる所（10 ms の窓のピークで見て、最後に 50 ms のフェードを足す）を TSV に書く |
| `plan` | 段ごと・鍵ごとの表、段の間引きの案（3・4・5・6・8 段）、メモリにのせたときの大きさ（f32・i16）、頭だけのせる形の見積もり |
| `list` | 案に入るサンプルの名前と、切る長さ（`convert.sh` と `load` の入力） |
| `load` | 名前の一覧のファイルを i16 か f32 のメモリにのせ、かかった時間と、前後の VmRSS を出す（`std::thread::scope` で N 本） |
| `evict` | 一覧のファイルだけ、PC のキャッシュから外してもらう（`posix_fadvise(DONTNEED)`。sudo なし、PC 全体の設定は変えない） |

`convert.sh` は ffmpeg と oggenc で配る形（FLAC 16 bit、Ogg Vorbis q6・q4、そのまま・切った物）に作りなおします。
`bench.sh` は `load` を 1 回ごとに別のプロセスで走らせ、最小・中央・最大を出します。
`sizes.sh` は配る形ごとの合計の大きさを出します。

## 走らせかた

素材は信用しない物として扱います（読むだけ。その中で何も走らせない）。

```bash
export CARGO_TARGET_DIR=target/r4-spike-load
cargo build --release -p sample-load
B=$CARGO_TARGET_DIR/release/sample-load
SRC=~/.cache/meloet/sounds-src/salamander/SalamanderGrandPiano-SFZ+FLAC-V3+20200602
SFZ="$SRC/SalamanderGrandPiano-V3+20200602.sfz"
W=~/.cache/meloet/sounds-src/salamander-work
mkdir -p $W/lists

$B sfz "$SFZ"
$B scan "$SFZ" $W/scan.tsv 8
$B plan "$SFZ" $W/scan.tsv > $W/plan.md

# 4 段と 6 段の案に入る物（リリース音・共鳴・ペダルつき）を作りなおす
for t in none 60 40; do $B list "$SFZ" $W/scan.tsv 4,6 extras $t > $W/lists/union-$t.tsv; done
for n in 4 6; do
  $B list "$SFZ" $W/scan.tsv $n extras none > $W/lists/plan$n.tsv
  $B list "$SFZ" $W/scan.tsv $n main none > $W/lists/plan$n-main.tsv
done
for f in flac16 ogg6 ogg4; do
  spikes/sample-load/convert.sh "$SRC/samples" $W/lists/union-none.tsv $W/$f-full $f full
  spikes/sample-load/convert.sh "$SRC/samples" $W/lists/union-60.tsv   $W/$f-t60  $f trim
  spikes/sample-load/convert.sh "$SRC/samples" $W/lists/union-40.tsv   $W/$f-t40  $f trim
done

spikes/sample-load/sizes.sh "$SRC/samples" $W > $W/sizes.tsv
spikes/sample-load/bench.sh $B "$SRC/samples" $W warm 3 > $W/bench-warm.tsv
spikes/sample-load/bench.sh $B "$SRC/samples" $W cold 3 > $W/bench-cold.tsv

# 1 回だけ読む（出力: 場所、拡張子、型、スレッド、数、ファイル MiB、音の秒、メモリ MiB、ms、前の RSS、後の RSS、VmHWM、検算）
$B load $W/flac16-full flac $W/lists/plan4.tsv i16 8
```

## 段の選びかた

1〜127 を N 等分し、それぞれの上の端にいちばん近い `hivel` の段を残します（いちばん上は必ず v16）。
残した段は「下の残した段の上の端 ＋ 1」から自分の上の端までを受けもつので、サンプルは録ったときより強く鳴らされることがありません
（弱く鳴らすだけ。雑音を持ち上げない）。

## 依存

`symphonia` 0.6.1（MPL-2.0。書きかえない。`flac`・`vorbis`・`ogg` だけ）、`libc`。
