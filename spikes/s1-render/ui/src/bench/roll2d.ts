import { C, NOTE_H, ROW, RULER_H, VEL_H, TPB, TPBAR, isBlack, lowerBound, velShade, type Notes, type Roll, type View } from "./model";

const SHADES = [C.g8, C.g9, C.g10];

export function makeRoll2d(canvas: HTMLCanvasElement, notes: Notes): Roll {
  const ctx = canvas.getContext("2d", { alpha: false })!;
  let w = 0, h = 0;
  return {
    resize(cw, ch, dpr) {
      w = cw; h = ch;
      canvas.width = Math.round(cw * dpr); canvas.height = Math.round(ch * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    },
    draw(v) {
      const gridH = h - RULER_H - VEL_H;
      const rows = Math.ceil(gridH / ROW);
      const x1 = v.x0 + w / v.pxPerTick;
      const X = (t: number) => (t - v.x0) * v.pxPerTick;

      ctx.fillStyle = C.g1; ctx.fillRect(0, 0, w, h);
      for (let r = 0; r < rows; r++) {
        if (!isBlack(v.topPitch - r)) { ctx.fillStyle = C.g2; ctx.fillRect(0, RULER_H + r * ROW, w, ROW); }
      }
      // Beat lines only when they are at least 6px apart.
      const step = TPB * v.pxPerTick >= 6 ? TPB : TPBAR * Math.max(1, Math.ceil(6 / (TPBAR * v.pxPerTick)));
      ctx.fillStyle = C.g4;
      for (let t = Math.floor(v.x0 / step) * step; t < x1; t += step) {
        ctx.fillStyle = t % TPBAR === 0 ? C.g5 : C.g4;
        ctx.fillRect(Math.round(X(t)), RULER_H, 1, gridH);
      }
      ctx.fillStyle = C.g3; ctx.fillRect(0, 0, w, RULER_H);
      ctx.fillStyle = C.g8; ctx.font = "12px 'IBM Plex Mono', monospace"; ctx.textBaseline = "top";
      const barStep = Math.max(1, Math.ceil(40 / (TPBAR * v.pxPerTick)));
      for (let b = Math.floor(v.x0 / TPBAR); b * TPBAR < x1; b += 1) {
        if (b % barStep === 0) ctx.fillText(String(b + 1), Math.round(X(b * TPBAR)) + 4, 4);
      }

      const lo = lowerBound(notes.start, v.x0 - notes.maxLen);
      const hi = lowerBound(notes.start, x1);
      const velTop = h - VEL_H;
      for (let shade = 0; shade < 3; shade++) {
        ctx.fillStyle = SHADES[shade];
        for (let i = lo; i < hi; i++) {
          if (velShade(notes.vel[i]) !== shade) continue;
          const xs = X(notes.start[i]), xe = X(notes.start[i] + notes.len[i]);
          if (xe < 0) continue;
          const r = v.topPitch - notes.pitch[i];
          if (r >= 0 && r < rows) ctx.fillRect(xs, RULER_H + r * ROW, Math.max(1, xe - xs), NOTE_H);
          const vh = (notes.vel[i] / 127) * (VEL_H - 8);
          ctx.fillRect(xs, velTop + VEL_H - vh, 2, vh);
        }
      }
      ctx.fillStyle = C.g10;
      for (let i = lo; i < hi; i++) {
        const r = v.topPitch - notes.pitch[i];
        if (r >= 0 && r < rows) ctx.fillRect(X(notes.start[i]), RULER_H + r * ROW, 2, NOTE_H);
      }
      ctx.fillStyle = C.g6; ctx.fillRect(0, velTop, w, 1);
      ctx.fillStyle = C.accent; ctx.fillRect(Math.round(X(v.playTick)), 0, 1, h);
    },
  };
}
