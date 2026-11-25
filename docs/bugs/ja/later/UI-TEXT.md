# UI-TEXT: 後で見たい点

- ui/src/text/ja.ts — 画面が一度も読まない 16 個のキーがあります(`"key"` を ui/src/text/ 以外ですべて検索し、テンプレートで作られる接頭辞は使われているものとして数えました): `msg.bufferFixedDetail`, `msg.countingIn`, `msg.formatSwitch`, `msg.pluginScanFailed`, `msg.cpuOverload`, `msg.notesNotMoved`, `msg.notesNotDuplicated`, `msg.takeDeleted`, `plugin.loadFailed`, `button.windows`, `button.switchFormat`, `button.keepRemoved`, `arrange.addTrackKinds`, `status.windows`, `msg.exporting`, `export.noClipping`。いくつかは仕様 6 の場面です(例: "Buffer is fixed (deep in the sound settings)"、CPU の過負荷)。各場面が別の方法で表示されているか、単に作られていないかを確かめてください。
- ui/src/text/en.ts:1551 — `export.midiVocal` は "voice track" と書いていますが、他の英語の文面はすべて、そして glossary.md も "vocal track" と書いています(日本語の元は「声のトラック」で、他の箇所は「歌のトラック」)。
- ui/src/text/en.ts:1193,1573 — `clipParams.pitchValue` / `msg.pitchDragging` の "{semitones} semitones {cents} cents" には単数形がありません("+1 semitones · +1 cents" になります)。数が二つある文面なので、`n()` の組だけでは直せません。
- ui/src/text/en.ts:2440-2457 — 音階のモード名は小文字です("major"、"minor pentatonic")。「C メジャー」→ "C major" ですが、同じ語は音階の一覧の行にもあり、そこでは "Dorian" や "Mixolydian" と並んで小文字で表示されます。
