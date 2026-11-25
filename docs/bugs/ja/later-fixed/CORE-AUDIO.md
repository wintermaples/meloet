# CORE-AUDIO — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/vocalrec.rs:157-161 — 音声ファイルの `write_all` が途中で失敗すると（空き容量がないなど）、後のブロックも書き込まれます。その後空きが戻ると、途中で短く書けた後のバイトがフレームとずれます（24 ビットではノイズになります）。また `frames` は、ファイルに入っている量より少なく数えます。録音はこの直後に止まります（`VocalsCannotGoOn`）ので、影響は短い区間です。
- crates/meloet-core/src/audiofiles/keeper.rs:620-637 — 復号したコピーはできたのに、ピークのファイルを書けないと、`finish` がエラーを返し、`Done { ok: false }` によって元ファイルが `Missing` になります。再生はできているのに、クリップは見つからないと表示されます。
- crates/meloet-core/src/readahead.rs:358-371 (with service.rs:1852-1874) — 小さな競合です。`store.open` が失敗してから `unopened.fetch_or` までの間に、ファイルの準備ができることがあります。すると `follow_store` は未開の座席を見つけられず、曲または保管庫がまた変わるまで、クリップは無音のままになります。
- crates/meloet-core/src/audiofiles/wav.rs:331-353 — `CacheWav::finish` は同期されていません。電源が落ちた後、名前を変えたが短いままの復号コピーが、正しいレートで開けてしまいます。すると `playable` はそれを準備完了と判定し、キャッシュを空にするまで、クリップの終わりが無音になります。
- crates/meloet-core/src/audiofiles/keeper.rs:538-546 — `CacheWav::create` の後に `Resample::new` が失敗すると、`.part` ファイルがキャッシュに残ります（早い `?` で、`abandon` を呼ばずに書き込み側が破棄されるためです）。
- crates/meloet-core/src/audiofiles.rs:482-491 — コピー中に保存が途中で止まると、`音/` フォルダに `<name>.part` が残ります。meloet はこれを掃除しません。
- crates/meloet-core/src/audiofiles.rs:308 — `bring_in` はコピーの名前を `fs::rename` で付けます。そのため、`find` と rename の間に同じ名前のファイルができると、それを置き換えてしまいます。`put_in_place` は `hard_link` を使い、これを防いでいます。
- crates/meloet-core/src/audiofiles/keeper.rs:465-467 — ヘッダーが実際より多いフレーム数を書いている flac は、起動のたびにピークが作り直されます（ピークのファイルの数が `source.frames` と一致しないためです）。
