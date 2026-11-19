// パラメータの欄の、画面だけが持つ状態。状態行も読む（「パラメータをさがしています…」「…を動かしています」。モック F-ParamSearch ・ F-ParamFew）。

export const paramUi = $state({
  /** さがす枠の文字。えらぶ子が変わると空に戻す。 */
  query: "",
  /** さがす枠に合う数。 */
  matches: 0,
  /** 欄の行をドラッグしている間: 子の名前とパラメータの名前。 */
  moving: null as { name: string; param: string } | null,
  /** 値を打ちこんでいる行（最初から入っているエフェクトは表の中の番号、プラグインは ParamRow.id）。 */
  typing: null as number | null,
});

/** 値を打ちこんでいる行を閉じる。閉じたら true。 */
export function stopTyping(): boolean {
  if (paramUi.typing === null) return false;
  paramUi.typing = null;
  return true;
}
