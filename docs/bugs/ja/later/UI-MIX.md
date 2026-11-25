# UI-MIX — 後で見たい点(バグではない)

- ui/src/components/mixer/control.svelte.ts:147 — ミキサーとパラメータのドラッグは `pointercancel` を扱いますが、`lostpointercapture` やウィンドウのぼけ(blur)は扱いません。ドラッグ中に `pointerup` が失われると(フォーカスが他に移った場合など)、`press` が残り、ホバーでコントロールが動き続け、次の押下は無視されます。テストでは再現していません。他の帯も同じ作りです。
- ui/src/components/plugins/PluginPlaces.svelte:15 — `placesReveal.pending` は、場所の部分が作られるときにだけ読まれます。詳細設定がすでに開いていて「プラグインの場所…」が押されると、何もスクロールされず、フラグは次に開くまで立ったままです。
- ui/src/components/params/ParamRow.svelte:111 — 組み込みパラメータのバーを押すと、押した点へ値が飛びます。一方、ミキサーのドラッグは、押した点からの距離だけ動きます。仕様 2 の「パラメータの行」と照らして確かめる価値があります。
