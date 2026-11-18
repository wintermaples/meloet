// 「meloet について」のダイアログ（仕様書 3 章）の、ひらいているか、どの段か、何をひらいているか。画面だけが持つ。
// 曲にも音にもさわらない、読むだけのダイアログなので、core に持たせない（約束を変えない）。
// Esc の順は、ほかのダイアログと同じ番（用語の説明やメニューの次）で、keys/attach.ts が aboutBack() を呼ぶ。

export type AboutSection = "sounds" | "fonts";

export const about = $state({
  open: false,
  /** 1: meloet について。2: オープンソースのライセンス。 */
  stage: 1 as 1 | 2,
  query: "",
  /** 音の素材と書体の見出し。はじめはひらいている（仕様書 3 章）。 */
  sections: { sounds: true, fonts: true } as Record<AboutSection, boolean>,
  /** ひらいているライセンスの見出し（licenseGroup の名前）。 */
  groups: [] as string[],
  /** 全文の枠をひらいている行（"名前@版"）。1 つずつ。 */
  part: null as string | null,
});

function reset(stage: 1 | 2): void {
  about.stage = stage;
  about.query = "";
  about.sections = { sounds: true, fonts: true };
  about.groups = [];
  about.part = null;
}

export function openAbout(): void {
  reset(1);
  about.open = true;
}

export function closeAbout(): void {
  about.open = false;
}

/** 2 段目へ。ひらくたびに、たたんだ形から始める。 */
export function showLicenses(): void {
  reset(2);
}

export function showFirstStage(): void {
  about.stage = 1;
}

/** Esc: 2 段目なら 1 段目へ、1 段目ならとじる。ひらいていなければ false（次の番へ）。 */
export function aboutBack(): boolean {
  if (!about.open) return false;
  if (about.stage === 2) showFirstStage();
  else closeAbout();
  return true;
}

export function toggleSection(s: AboutSection): void {
  about.sections[s] = !about.sections[s];
}

export function toggleGroup(group: string): void {
  about.groups = about.groups.includes(group) ? about.groups.filter((g) => g !== group) : [...about.groups, group];
}

export function togglePart(key: string): void {
  about.part = about.part === key ? null : key;
}
