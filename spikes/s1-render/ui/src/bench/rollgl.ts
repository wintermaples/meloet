import { NOTE_H, ROW, RULER_H, VEL_H, TPB, TPBAR, type Notes, type Roll, type View } from "./model";

// Background (rows, grid) is one full-screen pass; notes and velocity bars are one instanced pass each.
const BG_VS = `#version 300 es
void main(){ vec2 p = vec2((gl_VertexID<<1)&2, gl_VertexID&2); gl_Position = vec4(p*2.0-1.0,0,1); }`;
const BG_FS = `#version 300 es
precision highp float;
uniform vec2 uSize; uniform float uDpr, uX0, uPpt, uStep, uTop;
out vec4 o;
const vec3 G1=vec3(18.)/255., G2=vec3(22.)/255., G3=vec3(26.)/255., G4=vec3(34.)/255., G5=vec3(44.)/255., G6=vec3(58.)/255.;
void main(){
  vec2 p = vec2(gl_FragCoord.x, uSize.y*uDpr - gl_FragCoord.y) / uDpr;
  if (p.y < ${RULER_H}.) { o = vec4(G3,1); return; }
  float velTop = uSize.y - ${VEL_H}.;
  if (p.y >= velTop) { o = vec4(p.y < velTop+1. ? G6 : G1, 1); return; }
  int pitch = int(uTop) - int(floor((p.y - ${RULER_H}.) / ${ROW}.));
  int pc = ((pitch % 12) + 12) % 12;
  bool black = pc==1||pc==3||pc==6||pc==8||pc==10;
  vec3 c = black ? G1 : G2;
  float t = uX0 + p.x / uPpt;
  float k = floor(t / uStep) * uStep;
  if ((p.x - (k - uX0) * uPpt) < 1.0 / uDpr * uDpr) c = mod(k, ${TPBAR}.) < 0.5 ? G5 : G4;
  o = vec4(c,1);
}`;
const N_VS = `#version 300 es
layout(location=0) in float aStart; layout(location=1) in float aLen;
layout(location=2) in float aPitch; layout(location=3) in float aVel;
uniform vec2 uSize; uniform float uX0, uPpt, uTop, uMode; uniform vec4 uRect;
out float vShade; out float vLocalX;
void main(){
  vec2 corner = vec2(gl_VertexID & 1, (gl_VertexID >> 1) & 1);
  vec4 r;
  if (uMode < 0.5) {
    r = vec4((aStart - uX0) * uPpt, ${RULER_H}. + (uTop - aPitch) * ${ROW}., max(1.0, aLen * uPpt), ${NOTE_H}.);
  } else if (uMode < 1.5) {
    float vh = aVel / 127. * ${VEL_H - 8}.;
    r = vec4((aStart - uX0) * uPpt, uSize.y - vh, 2., vh);
  } else { r = uRect; }
  vShade = uMode > 1.5 ? 3. : (aVel < 70. ? 0. : aVel < 90. ? 1. : 2.);
  vLocalX = corner.x * r.z;
  vec2 px = r.xy + corner * r.zw;
  gl_Position = vec4(px.x / uSize.x * 2. - 1., 1. - px.y / uSize.y * 2., 0, 1);
}`;
const N_FS = `#version 300 es
precision highp float;
in float vShade; in float vLocalX; uniform float uMode; out vec4 o;
void main(){
  vec3 c = vShade < .5 ? vec3(154.) : vShade < 1.5 ? vec3(214.) : vShade < 2.5 ? vec3(242.) : vec3(255.,176.,0.);
  if (uMode < 0.5 && vLocalX < 2.) c = vec3(242.);
  o = vec4(c/255., 1);
}`;

function program(gl: WebGL2RenderingContext, vs: string, fs: string) {
  const p = gl.createProgram()!;
  for (const [type, src] of [[gl.VERTEX_SHADER, vs], [gl.FRAGMENT_SHADER, fs]] as const) {
    const s = gl.createShader(type)!;
    gl.shaderSource(s, src); gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? "shader");
    gl.attachShader(p, s);
  }
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? "link");
  return p;
}

export function makeRollGl(canvas: HTMLCanvasElement, ruler: HTMLCanvasElement, notes: Notes): Roll {
  const gl = canvas.getContext("webgl2", { antialias: false, alpha: false, powerPreference: "high-performance" });
  if (!gl) throw new Error("webgl2 unavailable");
  const rctx = ruler.getContext("2d")!;
  const bg = program(gl, BG_VS, BG_FS), np = program(gl, N_VS, N_FS);
  const U = (p: WebGLProgram, n: string) => gl.getUniformLocation(p, n);
  const vao = gl.createVertexArray(); gl.bindVertexArray(vao);
  const arrays = [notes.start, notes.len, Float32Array.from(notes.pitch), Float32Array.from(notes.vel)];
  arrays.forEach((a, i) => {
    const b = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, b);
    gl.bufferData(gl.ARRAY_BUFFER, a, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(i); gl.vertexAttribPointer(i, 1, gl.FLOAT, false, 0, 0); gl.vertexAttribDivisor(i, 1);
  });
  let w = 0, h = 0, dpr = 1;
  return {
    resize(cw, ch, d) {
      w = cw; h = ch; dpr = d;
      canvas.width = Math.round(cw * d); canvas.height = Math.round(ch * d);
      ruler.width = Math.round(cw * d); ruler.height = Math.round(RULER_H * d);
      rctx.setTransform(d, 0, 0, d, 0, 0);
      gl.viewport(0, 0, canvas.width, canvas.height);
    },
    draw(v: View) {
      const step = TPB * v.pxPerTick >= 6 ? TPB : TPBAR * Math.max(1, Math.ceil(6 / (TPBAR * v.pxPerTick)));
      gl.useProgram(bg);
      gl.uniform2f(U(bg, "uSize"), w, h); gl.uniform1f(U(bg, "uDpr"), dpr);
      gl.uniform1f(U(bg, "uX0"), v.x0); gl.uniform1f(U(bg, "uPpt"), v.pxPerTick);
      gl.uniform1f(U(bg, "uStep"), step); gl.uniform1f(U(bg, "uTop"), v.topPitch);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      gl.useProgram(np);
      gl.uniform2f(U(np, "uSize"), w, h); gl.uniform1f(U(np, "uX0"), v.x0);
      gl.uniform1f(U(np, "uPpt"), v.pxPerTick); gl.uniform1f(U(np, "uTop"), v.topPitch);
      gl.enable(gl.SCISSOR_TEST);
      gl.scissor(0, Math.round(VEL_H * dpr), canvas.width, Math.round((h - VEL_H - RULER_H) * dpr));
      gl.uniform1f(U(np, "uMode"), 0); gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, notes.count);
      gl.scissor(0, 0, canvas.width, Math.round((VEL_H - 1) * dpr));
      gl.uniform1f(U(np, "uMode"), 1); gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, notes.count);
      gl.disable(gl.SCISSOR_TEST);
      gl.uniform1f(U(np, "uMode"), 2);
      gl.uniform4f(U(np, "uRect"), Math.round((v.playTick - v.x0) * v.pxPerTick), 0, 1, h);
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, 1);

      rctx.fillStyle = "#1A1A1A"; rctx.fillRect(0, 0, w, RULER_H);
      rctx.fillStyle = "#9A9A9A"; rctx.font = "12px 'IBM Plex Mono', monospace"; rctx.textBaseline = "top";
      const barStep = Math.max(1, Math.ceil(40 / (TPBAR * v.pxPerTick)));
      const x1 = v.x0 + w / v.pxPerTick;
      for (let b = Math.floor(v.x0 / TPBAR); b * TPBAR < x1; b++) {
        if (b % barStep === 0) rctx.fillText(String(b + 1), Math.round((b * TPBAR - v.x0) * v.pxPerTick) + 4, 4);
      }
    },
  };
}
