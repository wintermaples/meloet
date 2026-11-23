// Whether the 「meloet について」 dialog (spec 3) is open, which page, and what is open in it. Only the screen holds it.
// It is a read-only dialog that touches neither the song nor the sound, so core does not hold it (the protocol does not change).
// In the Esc order it takes the same turn as other dialogs (after term tips and menus); keys/attach.ts calls aboutBack().

export type AboutSection = "sounds" | "fonts";

export const about = $state({
  open: false,
  /** 1: about meloet. 2: open source licenses. */
  stage: 1 as 1 | 2,
  query: "",
  /** Headings of the sound pack and fonts. Folded at first (the owner's decision; spec 3 says 「はじめからひらいている」). */
  sections: { sounds: false, fonts: false } as Record<AboutSection, boolean>,
  /** The open license heading (the name of licenseGroup). */
  groups: [] as string[],
  /** The row whose full text box is open ("name@version"). One at a time. */
  part: null as string | null,
});

function reset(stage: 1 | 2): void {
  about.stage = stage;
  about.query = "";
  about.sections = { sounds: false, fonts: false };
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

/** To the second page. Each time it opens, start folded. */
export function showLicenses(): void {
  reset(2);
}

export function showFirstStage(): void {
  about.stage = 1;
}

/** Esc: on the second page go to the first, on the first page close. false if not open (next turn). */
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
