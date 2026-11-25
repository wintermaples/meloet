# UI-MIX — 2 回目で直した点 (2026-11-25)

- ui/src/components/plugins/PluginPlaces.svelte:15 — `placesReveal.pending` は、場所の部分が作られるときにだけ読まれます。詳細設定がすでに開いていて「プラグインの場所…」が押されると、何もスクロールされず、フラグは次に開くまで立ったままです。
