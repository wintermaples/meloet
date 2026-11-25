# UI-MIX — fixed in the second pass (2026-11-25)

- ui/src/components/plugins/PluginPlaces.svelte:15 — `placesReveal.pending` is read only when the places part is made; if the advanced settings are already open when 「プラグインの場所…」 is pressed, nothing scrolls and the flag stays set until the next opening.
