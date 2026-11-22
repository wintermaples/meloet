# 音の素材を取りにいく（HTTP、TLS、続きから、置き場所、たしかめ）

調べた日: 2026-11-22。調べた人: meloet-research。コードは書いていません。`cargo tree` は動かせなかったので、依存の「数」は数えていません（下に書きました）。

## 1. 結論（先に）

| # | 結論 | 確かさ |
|---|---|---|
| 1 | HTTP は **`ureq` 3.x**（blocking、MIT OR Apache-2.0）がいちばん合う。ほかのスレッドで 1 本だけ使うので tokio はいらない。`reqwest` でも許しているライセンスの中に収まるが、木が大きい。 | A（ライセンス、機能）／C（大きさの比べ）|
| 2 | TLS は **rustls ＋ ring**（または aws-lc-rs）。どちらも deny.toml の許しの中。**OpenSSL の名前は出てこない**。証明書は **PC の証明書（rustls-platform-verifier）を先に、だめなら webpki-roots**。 | A |
| 3 | GitHub の Release の添付は、github.com から別ホスト（`release-assets.githubusercontent.com`）へ 302。そこは Range が通ると見られるが、**公式の文書にはない**。期限つきの URL は約 5 分〜1 時間の報告があり、切れると 403。 | C（一次情報は見つからなかった）|
| 4 | 空きは、すでに入っている `libc` の `statvfs` か、`rustix`（入っている 1.1.6、`fs` の機能）。 | A（rustix）|
| 5 | 置き場所は **`XDG_DATA_HOME`**（`~/.local/share/meloet/sounds/`）をすすめる。 | A（決まりの文）／C（すすめ）|
| 6 | ファイルごとの sha256 は、目録を信じられるなら足りる。**目録に minisign の署名**を足すのが安い（`minisign-verify`、MIT、依存なし）。 | A |
| 7 | 1 日 1 回は Sparkle の既定と同じ。User-Agent は `meloet/<版>` だけ。 | A（Sparkle）|

## 2. HTTP の部品

| 部品 | ライセンス | TLS の既定 | 続きから・リダイレクト・プロキシ | 大きさ |
|---|---|---|---|---|
| `ureq` 3.5.2 | MIT OR Apache-2.0 | 既定の機能は `rustls`（ring ＋ webpki-roots）と `gzip`。`platform-verifier` の機能で PC の証明書も使える | Range は自分でヘッダを足す。リダイレクトは既定 10 回。`Authorization` は引き継がない。プロキシは `ALL_PROXY` → `HTTPS_PROXY` → `HTTP_PROXY`（小文字も）と `NO_PROXY` を、`Config::default()` が読む。`https_only(true)` でリダイレクト先も HTTPS だけ。`Body::as_reader()` で `std::io::Read` | 必ず入る依存は base64、log、percent-encoding、ureq-proto、utf8-zero だけ。lib.rs の表示で、依存ぜんぶで約 425K 行 |
| `reqwest` 0.13.5 | MIT OR Apache-2.0 | 0.13 から既定は rustls ＋ **aws-lc-rs**、証明書は **rustls-platform-verifier**（webpki-roots は既定ではない）。`rustls-no-provider` で ring などに替えられる | リダイレクト 10 回。プロキシは `HTTP_PROXY` ・ `HTTPS_PROXY` ・ `ALL_PROXY` と小文字を既定で読む（`system-proxy`）。NO_PROXY は reqwest の頁に書かれていないが、中で使う hyper-util の `Matcher::from_env` は読む（そこからの推論）。**Referer を既定で送る**（`referer(false)` で止める）。User-Agent は既定では送らない | lib.rs の表示で依存ぜんぶ約 839K 行。`blocking` は中で tokio を使う |
| `hyper` を直に | MIT | hyper-rustls（Apache-2.0 OR ISC OR MIT）などを自分で組む | リダイレクトも Range も全部自分で書く | 小さいが、書く量が多い。おすすめしない |
| `minreq` 3.0.0 | ISC | `https-rustls`（rustls ＋ webpki-roots） | 軽い。ただし機能が少ない（大きなファイルの読み出しの細かい制御は未確認）| 小さい |
| `attohttpc` 0.32.0 | **MPL-2.0** | 既定は native-tls。rustls も選べる | blocking。MPL-2.0 は「ファイルを書きかえない」なら可 | 中 |

- いまの `Cargo.lock` には `reqwest` 0.13.5 があるが、`rustls`・`ring`・`aws-lc`・`native-tls`・`h2` は 1 つも入っていない（Cargo.lock を読んで確かめた）。つまり、どの TLS を選んでも新しく入る。
- 依存の「数」は数えていません。自分の見積もり（C）では ureq ＋ ring が数十、reqwest ＋ aws-lc-rs はその数倍。決める前に `cargo tree -p <名前> | wc -l` で数えてください。
- 確かさ: ライセンス、機能は A（各リポジトリの Cargo.toml、docs.rs）。行数は B（lib.rs）。

## 3. TLS の部品とライセンス

meloet の許し（`deny.toml` と `licenses/check.mjs`）: MIT、Apache-2.0、Apache-2.0 WITH LLVM-exception、BSD-2/3-Clause、ISC、Zlib、0BSD、Unicode-3.0、CC0-1.0、MPL-2.0、BSL-1.0、Unlicense、CDLA-Permissive-2.0（check.mjs は ほかに Unicode-DFS-2016、BlueOak-1.0.0、Python-2.0、CC-BY-4.0、OFL-1.1）。**OpenSSL、SSLeay、MIT-0 は許しに入っていない**。

| 部品 | ライセンス（Cargo.toml の文）| 許しに入るか |
|---|---|---|
| `rustls` 0.23.46 | Apache-2.0 OR ISC OR MIT | 入る |
| `ring` 0.17.14（crates.io の最新）| Apache-2.0 AND ISC（LICENSE: 新しい所は ISC、BoringSSL 由来は Apache-2.0、once_cell 由来は Apache/MIT）| 入る |
| `aws-lc-rs` 1.19.0 | ISC AND (Apache-2.0 OR ISC) | 入る |
| `aws-lc-sys` 0.47.0（C のソース）| ISC AND (Apache-2.0 OR ISC) AND Apache-2.0 AND MIT AND BSD-3-Clause AND (Apache-2.0 OR ISC OR MIT) AND (Apache-2.0 OR ISC OR MIT-0) | 入る。MIT-0 は OR の中で、Apache-2.0 を選べる。LICENSE には OpenSSL と SSLeay の名前が出るが、「OpenSSL 由来の部分は Apache-2.0」と書いてある（OpenSSL 独自のライセンスの本文はない）|
| `rustls-webpki` | ISC | 入る |
| `webpki-roots` 1.0.12 | CDLA-Permissive-2.0 | 入る（`deny.toml` に入っている）|
| `rustls-native-certs` 0.8.5 | Apache-2.0 OR ISC OR MIT | 入る（Linux で `openssl-probe` を使う）|
| `rustls-platform-verifier` 0.7.1 | MIT OR Apache-2.0 | 入る。Linux では rustls-native-certs と webpki で確かめる。**失効の確認（CRL）はしない**。PC の証明書は起動のとき 1 回だけ読む |
| `native-tls` 0.2.18 | MIT OR Apache-2.0 | 入る。ただし Linux では `openssl` クレートで PC の libssl につなぐ。OpenSSL 3 は Apache-2.0、1.1.1 以前は OpenSSL ＋ SSLeay の二本立てで許しの外。PC の libssl の版しだいになる |

- ビルドに C が要る。aws-lc の文書では、FIPS でない版は C/C++ のコンパイラだけでよい（bindgen、cmake、Go はいらない）。rustls の文書は「cmake が要る」と書く。食いちがいがある（新しい aws-lc の文書を採る）。meloet は `meloet-stretch` で C++ をもう組んでいるので、問題は小さい。
- **すすめ**: ring か aws-lc-rs か。どちらも許しの中。ring は小さく組みやすい。aws-lc-rs は reqwest 0.13 の既定で、C のコードが多い。ureq なら ring が既定で付いてくる。native-tls は PC の libssl に左右されるので、すすめない。
- 確かさ: A（Cargo.toml、LICENSE、docs.rs、openssl-library.org）。

## 4. 続きから取る（Range と GitHub）

**HTTP の決まり（RFC 9110、A）**
- サーバは `Range` を**無視してよい**（200 で全部返してよい）。206 は成功、416 は範囲が合わない。
- `If-Range` は、ETag か Last-Modified が今の物と合うときだけ Range を使わせる。合わないとき、サーバは Range を無視し、200 で全部返す。Last-Modified は「強い」ものだけ。
- 毎回、返事が 206 か 200 かを見る。200 なら、はじめから書きなおす。

**GitHub（公式の文書でわかる事 A）**
- Release の添付は、1 つの Release に 1000 個まで、1 ファイルは 2 GiB 未満。**合計の大きさと転送量に上限はない**（docs.github.com「About releases」）。
- 認証なしの REST API は 1 時間 60 回（IP ごと）。ただし、`github.com/.../releases/download/...` が数えられるかは、この文書に書かれていない。
- 添付を API で取ると 200 か 302 が返る（クライアントは両方に備える）。API には User-Agent が必須（なしは拒否）。ブラウザ用の `releases/download` の URL ではこの規則に書かれていないが、付けておく。
- `releases/latest/download/<file>` は公式に書いてある URL（最新の Release の添付）。

**公式の文書に書いていない事（B/C。一次情報は見つからなかった）**
- 302 の先は `release-assets.githubusercontent.com`（前は `objects.githubusercontent.com`）。URL に JWT と `se=...Z`（期限）がつく（GitHub community #169381、C）。
- 期限: 約 5 分（JWT の exp と nbf が 300 秒ちがい）という報告と、約 1 時間という報告がある（食いちがい。どちらも個人の投稿、C）。流れている最中の本体は、期限が切れても止まらない（C）。
- 期限が切れたあとの続きは **403**（200/206 ではない）（C）。いちど 618 `jwt:expired` が出た報告もある。直し方は、github.com の URL からもう一度たどって新しい URL をもらい、`Range: bytes=<持っている長さ>-` で続きを頼む（C）。
- 206 になるか: wget で GitHub の Release を取ると 206 になった例がある（C）。ETag と `Accept-Ranges` が出るかは、見つけた捕獲には両方そろった物がなかった。Azure Blob の ETag は出ている例がある（C）。
- **必ずやってほしい確かめ**: 本物の Release の 1 つに `curl -sIL` と `curl -r 0-99 -L` をかけて、302 の回数、`Accept-Ranges`、`ETag`、206 を見る。ここでは検査の道具がなく、確かめられなかった。

**meloet の場合の考え**（入力は上の C なので、確かさは C）
- 1 音色は平均 約 7.7 MB（240 MB ÷ 31）。回線が普通なら、期限（5 分）の中に取り終わる。Range は「あれば得」で、なくても、音色ごとの取り直しで足りる。
- 部分ファイル（`.part`）は、sha256 を確かめたら名前を変える。**If-Range に頼らず**、最後に sha256 で確かめて、合わなければ消して取りなおすのがいちばん強い。続きの 403/410 は、github.com の URL からやりなおす。
- 目録（`catalog.json`）は小さい。条件つき取得（`If-None-Match`）は、github.com 側の返事が出るかを実際に見てから決める。

## 5. ディスクの空き

| 部品 | ライセンス | 中身 |
|---|---|---|
| `libc`（入っている 0.2.191）| 入っている（`deny.toml` を通っている）| `libc::statvfs`。新しい部品なし |
| `rustix` 1.1.6（`Cargo.lock` に入っている）| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | `rustix::fs::statvfs(path) -> StatVfs`。`fs` の機能が要る。`f_bsize`、`f_frsize`、`f_bavail`、`f_bfree` などは `u64` |
| `nix` 0.32.0 | MIT | `fs` の機能（`fs = []`）で、libc の上に作る |
| `fs4` 1.1.0 | MIT OR Apache-2.0 | rustix の上に作る。`available_space` は、この頁では確かめられなかった（C）|
| `fs2` | 見ていない | 古い（C。確かめていない）|

- 空きは、POSIX の習いで `f_bavail × f_frsize`（一般のユーザーが使える分）。この習いは記憶（C）。
- すすめ: 新しい部品を足さず、`libc` か、すでにある `rustix` を使う。`cargo tree -e features -i rustix` で、`fs` の機能がもう立っているか確かめる。
- 足りるかの見かた（C）: 残りのバイト数 ＋ 余り（64 MB など）。足りなければ、取りはじめる前に出す。

## 6. 置き場所（XDG）

- 決まり（A、specifications.freedesktop.org）: `XDG_DATA_HOME` は「ユーザー用のデータ」（既定 `~/.local/share`）、`XDG_CACHE_HOME` は「**なくてもよい**（非必須）ユーザー用のデータ」（既定 `~/.cache`）、`XDG_STATE_HOME` は再起動をまたぐ状態（既定 `~/.local/state`）。空か相対の道は無視して既定を使う。
- 掃除の道具: BleachBit の FAQ は、`~/.cache` をそのまま消す使いかたが前提で、「消すと、取りなおしや作りなおしで遅くなる」と書く（A）。systemd の `tmpfiles.d` の文書（A）には、ユーザーの `~/.cache` を既定で掃除する決めは書かれていない（見つからなかった）。
- ほかのアプリ: umu-launcher は、取ってきた実行環境を `$XDG_DATA_HOME/umu` に残すとすすめている（GitHub の議論、C）。Hugging Face が `~/.cache` に置くことと、Steam が `~/.local/share/Steam` に置くことは、記憶（C）で、確かめていない。
- すすめ（C）: ここは meloet が動くのに**必要な**もので、240 MB を取りなおすのは重い（ネットがない所では取れない）。**`~/.local/share/meloet/sounds/` に置く**。ただし meloet はすでに `audiofiles.rs` の WAV の写しを `XDG_CACHE_HOME/meloet` に置いており、「消えても作りなおせる」物はそちらのままでよい。「取りなおせるが、消されると困る」ので DATA、という分けかた。

## 7. 取った物のたしかめ

- **ファイルごとの sha256 ＋ 大きさ** を目録に書くのは、とちゅうの欠け、ビット化けには十分（`sha2` は入っている）。
- ただし目録も、同じ Release から取る。**Release の書きかえ、アカウントの乗っとりが起きたら、目録ごと差し替えられる**。TLS は「GitHub につながっている」ことしか言わない（推論、C）。
- 署名: **`minisign-verify` 0.3.0**（MIT、依存なし、2026-09-25）で、目録の `.minisig` をたしかめる。公開鍵は、実行ファイルの中に入れる。秘密鍵は、Release を作る PC（や CI）の外（オフライン）に置く。`ed25519-dalek` 3.0.0（BSD-3-Clause、curve25519-dalek と sha2 0.11 に頼る）でも書けるが、署名の形式を自分で決める分、手間が多い。
- Tauri の updater も、署名を**切れない**ようにして minisign 系の鍵を使う。公開鍵は `tauri.conf.json` の中の文字列（ファイルの道ではだめ）。HTTPS は本番で強制（A、v2.tauri.app）。
- GitHub の「**immutable releases**」（A、docs.github.com）: 公開した Release のタグと添付は、書きかえも消すこともできない。attestation（証明）も自動で付く。これを使えば、Release の書きかえには強くなる。ただし、アカウントの乗っとりなどで新しい Release を出される事までは防げない。署名はその時のための物。
- 要るか: 音の素材は FLAC で、デコーダ（symphonia）は信頼できない入力として扱う事になる。署名があれば、その入口を自分たちの鍵の持ち主に絞れる。要らない、とも言える（TLS ＋ immutable releases ＋ sha256 だけ）。判断はオーナーさんの方針しだい。

## 8. 起動のときの見にいく回数と名前

- Sparkle の既定は **86400 秒（1 日）**、最短 1 時間（A、sparkle-project.org）。システム情報を送る機能は既定で切れている。Tauri の updater には、自動の間隔の決めはない（呼ぶのは自分、A）。Firefox などの間隔は、見られなかった。
- 名前を送らない: `ureq` の既定は `ureq/<版>`、`reqwest` は何も送らない（A）。meloet は、`meloet/<版>` だけを付ける（OS、機械の名前、利用者の名前を入れない。C）。reqwest なら `referer(false)` も。IP アドレスは GitHub に見える（これは避けられない。C）。
- 「見にいった時刻」を `XDG_STATE_HOME` か設定に残して、24 時間たつまで見にいかない形が、いちばん単純（C）。

## 9. meloet へのすすめ

1. **部品**: `ureq` 3.x（機能は `rustls`（ring）＋ `platform-verifier`）。専用の 1 スレッドで動かす。`https_only(true)`、`max_redirects` は 10 のまま、User-Agent は `meloet/<版>`。`reqwest` は、すでに tauri 側で入っていても、meloet では使わない。
2. **証明書**: 先に `RootCerts::PlatformVerifier`、PC の証明書が読めなかったときだけ `WebPki`（webpki-roots）。これで会社のプロキシ（中身を見る型）にも合う。
3. **続きから**: 音色ごとに `*.part` ＋ Range（206 のとき）。とちゅうで 403/410 が来たら、github.com の URL から、もう一度たどる。仕上げは必ず sha256 と大きさで確かめ、合わなければ消して取りなおす。If-Range は付けても付けなくてもよい（頼らない）。
4. **置き場所**: `$XDG_DATA_HOME/meloet/sounds/<音色>/<版>/`（なければ `~/.local/share`）。目録は同じ下に。WAV の写しは今のまま `XDG_CACHE_HOME`。
5. **空き**: `libc::statvfs`（か入っている rustix）。取りはじめる前に出す。
6. **たしかめ**: 目録（音色、版、大きさ、sha256）＋ `minisign-verify` の署名（オーナーさんが決めたら）。Release は immutable にする。
7. **先にやる確かめ**: 本物の Release に `curl -sIL`、`curl -r 0-99 -L` をかけて、4 章の C の部分を A にする。それと `cargo tree` で依存の数を数える。

## 出どころ（見た日: 2026-11-22）

| URL | 書いてあった事 | 確かさ |
|---|---|---|
| /srv/meloet-agent/meloet/deny.toml、licenses/check.mjs、Cargo.lock | 許しているライセンス、reqwest 0.13.5・hyper 1.12.0・rustix 1.1.6・libc 0.2.191 は入っていて rustls・ring は入っていない | A |
| https://docs.rs/crate/reqwest/0.13.5/source/Cargo.toml | 機能、既定（default-tls = rustls）、ライセンス | A |
| https://docs.rs/reqwest/0.13.5/reqwest/ ・ https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md ・ …/src/async_impl/client.rs | 既定の rustls ＋ aws-lc ＋ platform-verifier、プロキシの環境変数、リダイレクト 10、Referer 既定、UA なし | A |
| https://docs.rs/hyper-util/latest/hyper_util/client/proxy/matcher/struct.Matcher.html | ALL/HTTPS/HTTP/NO_PROXY を読む | A |
| https://docs.rs/crate/ureq/latest/source/Cargo.toml ・ https://docs.rs/ureq/latest/ureq/ ・ …/config/struct.Config.html ・ github.com/algesten/ureq の src/run.rs、config.rs | 3.5.2、機能、プロキシ、リダイレクト 10、`https_only`、UA、blocking | A |
| https://lib.rs/crates/ureq 、/reqwest | 行数（約 425K、約 839K） | B |
| https://raw.githubusercontent.com/aws/aws-lc-rs/main/{aws-lc-rs,aws-lc-sys}/Cargo.toml と aws-lc-sys/LICENSE ・ https://aws.github.io/aws-lc-rs/requirements/linux.html | ライセンスの式、OpenSSL 由来は Apache-2.0、ビルドは C コンパイラだけ | A |
| https://raw.githubusercontent.com/briansmith/ring/main/{Cargo.toml,LICENSE} ・ https://crates.io/api/v1/crates/{ring,rustls,aws-lc-rs,minisign-verify,ed25519-dalek} | ring の式 Apache-2.0 AND ISC、最新の版と日付 | A |
| https://docs.rs/rustls/latest/rustls/ ・ rustls/rustls・webpki・webpki-roots・rustls-native-certs・rustls-platform-verifier の各 Cargo.toml ・ https://docs.rs/rustls-platform-verifier/0.7.1/… ・ https://docs.rs/rustls-native-certs/0.8.5/… | ライセンス、既定の暗号の部品、Linux の証明書の探しかた、CRL なし、SSL_CERT_FILE | A |
| https://openssl-library.org/source/license | OpenSSL 3 は Apache-2.0、1.1.1 以前は OpenSSL ＋ SSLeay | A |
| https://raw.githubusercontent.com/sfackler/rust-native-tls/master/Cargo.toml | 0.2.18、Linux は openssl に依存 | A |
| neonmoe/minreq・sbstp/attohttpc の Cargo.toml | ISC、MPL-2.0 | A |
| https://docs.rs/rustix/latest/rustix/fs/fn.statvfs.html ・ …/struct.StatVfs.html ・ rustix・nix・fs4-rs の Cargo.toml | statvfs の書きかた、ライセンス | A |
| https://specifications.freedesktop.org/basedir/latest/ | DATA・CACHE・STATE の意味、既定 | A |
| https://docs.bleachbit.org/doc/frequently-asked-questions.html ・ https://www.freedesktop.org/software/systemd/man/latest/tmpfiles.d.html | ~/.cache の掃除、tmpfiles が既定で ~/.cache を掃除するとは書かれていない | A |
| https://www.rfc-editor.org/rfc/rfc9110.html | Range・If-Range・206・416 | A |
| https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases | 添付 1000 個、2 GiB 未満、合計と転送量に上限なし | A |
| https://docs.github.com/en/rest/releases/assets ・ …/linking-to-releases ・ …/rate-limits-for-the-rest-api ・ …/getting-started-with-the-rest-api | 200/302、latest/download、60 回/時、User-Agent 必須。URL の期限と Range は書かれていない | A |
| https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases | 添付が書きかえ・削除できない、attestation | A |
| https://v2.tauri.app/plugin/updater/ | 署名は切れない、公開鍵は設定の文字列、TLS は本番で強制 | A |
| https://sparkle-project.org/documentation/customization/ | 1 日、最短 1 時間 | A |
| https://github.com/orgs/community/discussions/169381 | 302 → release-assets、JWT、約 300 秒、期限後 618、wget -c / curl -C - で続きが取れたとの報告 | C |
| https://github.com/AnyTogether/AnyPS5-Desktop/issues/187 | 約 1 時間の報告、期限切れの続きは 403、再取得して Range で続き（取り込み側の診断に争いあり） | C |
| https://github.com/orgs/community/discussions/167206 、/46420 ほか | 添付の URL の形（URL に JWT、期限、リポジトリ ID）。期限、Range の記述なし | C |
| Web 検索の結果（wget の 206 の例、urlquery の Azure Blob の ETag） | 206 になった例、ETag はあるが Accept-Ranges は未確認 | C |
| https://github.com/Open-Wine-Components/umu-launcher/pull/584 | ランタイムを $XDG_DATA_HOME/umu に残す話 | C |

## 10. わからなかった事、追加で調べるとよい事

- GitHub の `release-assets.githubusercontent.com` の **Range、ETag、期限、リダイレクトの回数**は、公式の文書では見つからなかった。本物の Release に `curl -sIL` をかけるのが早い。
- 認証なしの `releases/download` の回数の上限は、文書にない（API の 60 回/時とは別と見られるが、推論）。
- ureq でリダイレクト先に `Range` ヘッダが付くかは、コードでは「消す」とは見えなかったが、実際に試していない。
- 依存の数（`cargo tree`）、`rustix` の `fs` の機能がもう立っているか、`ureq` の ring ＋ platform-verifier の組みあわせでビルドが通るかは、試していない。
- aws-lc-rs の「cmake が要る」と「C だけでよい」の食いちがいは、実際のビルドで見たい。
- `ed25519-dalek` 3.0.0 と `minisign-verify` 0.3.0 は、日付が新しい（2026-07、2026-09）。使う前に、変更の一覧（CHANGELOG）を見る。
