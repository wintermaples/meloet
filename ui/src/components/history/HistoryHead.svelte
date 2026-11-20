<!-- 取り消しの履歴の帯の見出しの行の、名前より右（モック F-History）: 手の数といまの手、取り消す ・ やりなおすのボタン。 -->
<script lang="ts">
  import { platformOf } from "../../keys/keys";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";

  const history = $derived(current.state!.history);
  const head = $derived.by(() => {
    const { total, current: now, redo } = history;
    if (total === 0) return t("history.headEmpty", { total });
    if (redo === 0) return t("history.headLatest", { total, current: now });
    if (now === 0) return t("history.headAllUndone", { total, redo });
    return t("history.head", { total, current: now, redo });
  });
  const mod = platformOf(navigator) === "mac" ? t("key.cmd") : t("key.ctrl");
</script>

<span class="m-lbl" data-test="history-head">{head}</span>
<span class="m-spacer"></span>
<button type="button" class="m-btn" aria-disabled={history.current === 0} data-test="history-undo" onclick={() => history.current > 0 && send({ type: "undo" })}>
  {t("history.undo")} <span class="m-num m-lbl">{mod} {t("key.undo")}</span>
</button>
<button type="button" class="m-btn" aria-disabled={history.redo === 0} data-test="history-redo" onclick={() => history.redo > 0 && send({ type: "redo" })}>
  {t("history.redo")} <span class="m-num m-lbl">{mod} {t("key.shift")} {t("key.undo")}</span>
</button>
