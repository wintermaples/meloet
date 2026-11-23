import { FLOATS_PER_HATCH, type Batch, type PlacedWave } from "./batch";
import { WAVE_TEX_WIDTH, WaveTextures, type TexOps } from "./wave";

/** (x, y, r, g, b) */
const FLOATS_PER_CORNER = 5;

const VERTEX = `#version 300 es
in vec2 a_pos;
in vec3 a_color;
uniform vec2 u_size;
out vec3 v_color;
void main() {
  vec2 c = a_pos / u_size * 2.0 - 1.0;
  gl_Position = vec4(c.x, -c.y, 0.0, 1.0);
  v_color = a_color;
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in vec3 v_color;
out vec4 o_color;
void main() {
  o_color = vec4(v_color, 1.0);
}`;

// The corners come from the number of the vertex: the rectangle of a wave or a hatch is a uniform, so there is no buffer to fill.
const QUAD_VERTEX = `#version 300 es
uniform vec2 u_size;
uniform vec4 u_rect;
out vec2 v_px;
void main() {
  v_px = vec2((gl_VertexID & 1) == 0 ? u_rect.x : u_rect.z, (gl_VertexID & 2) == 0 ? u_rect.y : u_rect.w);
  vec2 c = v_px / u_size * 2.0 - 1.0;
  gl_Position = vec4(c.x, -c.y, 0.0, 1.0);
}`;

// For each pixel: the bar it lies in, the peaks under that bar (smallest and largest sample), and whether the pixel
// is between them. The fade is the one of ui/src/components/arrange/fade.ts (sin(x · π/2)); its line is drawn on top.
// Pixels of neither are left as they are (the clip's ground).
const WAVE_FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
precision highp isampler2D;
in vec2 v_px;
out vec4 o_color;
uniform isampler2D u_peaks;
uniform int u_stereo;
uniform ivec3 u_index;
uniform vec3 u_map;
uniform vec4 u_bars;
uniform vec2 u_y;
uniform float u_gain;
uniform vec4 u_fade;
uniform vec3 u_color;
uniform vec4 u_mid;
uniform vec4 u_line;
const int W = ${WAVE_TEX_WIDTH};
const int MOST = 16;
const float HALF_PI = 1.5707963267948966;
float rise(float t) {
  return t >= 1.0 ? 1.0 : (t > 0.0 ? sin(t * HALF_PI) : 0.0);
}
float fadeAt(float x) {
  float g = 1.0;
  if (u_fade.y > 0.0) g *= rise((x - u_fade.x) / u_fade.y);
  if (u_fade.w > 0.0) g *= rise((u_fade.z - x) / u_fade.w);
  return g;
}
// The middle of the pixel row where a fade's line is at the size g: the bottom row at 0, the top row at 1.
float lineY(float g) {
  return u_y.y - 0.5 - g * (u_y.y - u_y.x - 1.0);
}
bool between(float yc, float a, float b) {
  return yc >= min(a, b) - 0.5 && yc < max(a, b) + 0.5;
}
void main() {
  float x = floor(v_px.x);
  float y = floor(v_px.y);
  float yc = y + 0.5;
  if (u_line.a > 0.0) {
    float inEnd = u_fade.x + u_fade.y;
    if (u_fade.y > 0.0 && x + 1.0 > u_fade.x && x < inEnd) {
      float a = lineY(rise((max(x, u_fade.x) - u_fade.x) / u_fade.y));
      float b = lineY(rise((min(x + 1.0, inEnd) - u_fade.x) / u_fade.y));
      if (between(yc, a, b)) { o_color = vec4(u_line.rgb, 1.0); return; }
    }
    float outStart = u_fade.z - u_fade.w;
    if (u_fade.w > 0.0 && x + 1.0 > outStart && x < u_fade.z) {
      float a = lineY(rise((u_fade.z - max(x, outStart)) / u_fade.w));
      float b = lineY(rise((u_fade.z - min(x + 1.0, u_fade.z)) / u_fade.w));
      if (between(yc, a, b)) { o_color = vec4(u_line.rgb, 1.0); return; }
    }
  }
  float rel = x - u_bars.x;
  float k = floor(rel / u_bars.y);
  if (rel - k * u_bars.y < u_bars.z) {
    float bx = u_bars.x + k * u_bars.y;
    float a = u_map.y + (bx - u_map.x) * u_map.z;
    float b = a + u_bars.y * u_map.z;
    int i0 = u_index.x + int(floor(a));
    int i1 = u_index.x + int(ceil(b));
    i1 = min(max(i1, i0 + 1), i0 + MOST);
    i0 = max(i0, u_index.y);
    i1 = min(i1, u_index.z);
    if (i0 < i1) {
      int lo = 32767;
      int hi = -32768;
      for (int i = i0; i < i1; i++) {
        ivec4 t = texelFetch(u_peaks, ivec2(i % W, i / W), 0);
        lo = min(lo, t.r);
        hi = max(hi, t.g);
        if (u_stereo != 0) {
          lo = min(lo, t.b);
          hi = max(hi, t.a);
        }
      }
      float g = u_gain * fadeAt(bx + 0.5 * u_bars.z);
      float mid = 0.5 * (u_y.x + u_y.y);
      float halfH = 0.5 * (u_y.y - u_y.x);
      float t0 = mid - float(hi) / 32768.0 * g * halfH;
      float t1 = mid - float(lo) / 32768.0 * g * halfH;
      if (t1 - t0 < u_bars.w) {
        float c = 0.5 * (t0 + t1);
        t0 = c - 0.5 * u_bars.w;
        t1 = c + 0.5 * u_bars.w;
      }
      if (yc >= floor(t0 + 0.5) && yc < floor(t1 + 0.5)) { o_color = vec4(u_color, 1.0); return; }
    }
  }
  if (u_mid.a > 0.0 && y == floor(0.5 * (u_y.x + u_y.y))) { o_color = vec4(u_mid.rgb, 1.0); return; }
  discard;
}`;

const WAVE_UNIFORMS = ["u_size", "u_rect", "u_peaks", "u_stereo", "u_index", "u_map", "u_bars", "u_y", "u_gain", "u_fade", "u_color", "u_mid", "u_line"] as const;
type WaveUniform = (typeof WAVE_UNIFORMS)[number];

// Batch.hatch: u_hatch is (pitch, shift). Pixels are counted on the picture (v_px), not gl_FragCoord, so a picture
// drawn lower in its canvas (`below`) has the same lines.
const HATCH_FRAGMENT = `#version 300 es
precision highp float;
precision highp int;
in vec2 v_px;
out vec4 o_color;
uniform ivec2 u_hatch;
uniform vec3 u_color;
void main() {
  ivec2 p = ivec2(floor(v_px));
  if ((p.x + p.y + u_hatch.y) % u_hatch.x != 0) discard;
  o_color = vec4(u_color, 1.0);
}`;

const HATCH_UNIFORMS = ["u_size", "u_rect", "u_hatch", "u_color"] as const;
type HatchUniform = (typeof HATCH_UNIFORMS)[number];

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(`gl: shader: ${gl.getShaderInfoLog(shader)}`);
  }
  return shader;
}

/** Draws a Batch: one buffer upload and one draw call per picture. */
export class Renderer {
  private readonly gl: WebGL2RenderingContext;
  private size: WebGLUniformLocation | null = null;
  private capacity = 0;
  private program: WebGLProgram | null = null;
  private vao: WebGLVertexArrayObject | null = null;
  private wave: { program: WebGLProgram; vao: WebGLVertexArrayObject | null; at: Record<WaveUniform, WebGLUniformLocation | null> } | null = null;
  private hatch: { program: WebGLProgram; vao: WebGLVertexArrayObject | null; at: Record<HatchUniform, WebGLUniformLocation | null> } | null = null;
  /** The program (and its vertex array) in use. */
  private using: "rects" | "wave" | "hatch" = "rects";
  private readonly textures: WaveTextures<WebGLTexture>;
  /** How long making it took (ms): getContext, then setUp. */
  readonly madeMs: { context: number; program: number };

  /** `opaque`: the picture covers the whole canvas, so the page behind it need not show through. */
  constructor(canvas: HTMLCanvasElement, opaque: boolean) {
    const t0 = performance.now();
    const gl = canvas.getContext("webgl2", {
      alpha: !opaque,
      antialias: false,
      depth: false,
      stencil: false,
      // WebKitGTK's snapshot (MELOET_SNAPSHOT) reads the canvas after the frame was shown;
      // without this it finds the buffer already cleared and the PNG lacks the whole surface.
      preserveDrawingBuffer: true,
    });
    if (!gl) throw new Error("gl: WebGL2 is not available");
    this.gl = gl;
    this.textures = new WaveTextures(this.texOps());
    const t1 = performance.now();
    this.setUp();
    this.madeMs = { context: t1 - t0, program: performance.now() - t1 };
  }

  private link(vertex: string, fragment: string): WebGLProgram {
    const gl = this.gl;
    const program = gl.createProgram()!;
    gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, vertex));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fragment));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      throw new Error(`gl: program: ${gl.getProgramInfoLog(program)}`);
    }
    return program;
  }

  /** Also called after the context comes back from a loss. */
  setUp(): void {
    const gl = this.gl;
    // The wave program is made with the context, not with the first wave: a compile stops the page for a moment,
    // and the first wave may come while somebody records.
    const wave = this.link(QUAD_VERTEX, WAVE_FRAGMENT);
    const at = {} as Record<WaveUniform, WebGLUniformLocation | null>;
    for (const name of WAVE_UNIFORMS) at[name] = gl.getUniformLocation(wave, name);
    this.wave = { program: wave, vao: gl.createVertexArray(), at };
    gl.useProgram(wave);
    gl.uniform1i(at.u_peaks, 0);
    this.textures.forget();

    const hatch = this.link(QUAD_VERTEX, HATCH_FRAGMENT);
    const hatchAt = {} as Record<HatchUniform, WebGLUniformLocation | null>;
    for (const name of HATCH_UNIFORMS) hatchAt[name] = gl.getUniformLocation(hatch, name);
    this.hatch = { program: hatch, vao: gl.createVertexArray(), at: hatchAt };

    const program = this.link(VERTEX, FRAGMENT);
    this.program = program;
    gl.useProgram(program);
    this.using = "rects";
    this.size = gl.getUniformLocation(program, "u_size");

    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    this.capacity = 0;
    const stride = 5 * Float32Array.BYTES_PER_ELEMENT;
    const pos = gl.getAttribLocation(program, "a_pos");
    const color = gl.getAttribLocation(program, "a_color");
    gl.enableVertexAttribArray(pos);
    gl.vertexAttribPointer(pos, 2, gl.FLOAT, false, stride, 0);
    gl.enableVertexAttribArray(color);
    gl.vertexAttribPointer(color, 3, gl.FLOAT, false, stride, 2 * Float32Array.BYTES_PER_ELEMENT);
    gl.disable(gl.BLEND);
    gl.clearColor(0, 0, 0, 0);
  }

  /** `below`: rows of the canvas under the picture, when the canvas is higher than the picture (the picture is at its top left). */
  draw(batch: Batch, width: number, height: number, below = 0): void {
    const gl = this.gl;
    gl.viewport(0, below, width, height);
    this.useRects();
    gl.uniform2f(this.size, width, height);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (batch.length > 0) {
      if (batch.data.length > this.capacity) {
        gl.bufferData(gl.ARRAY_BUFFER, batch.data.byteLength, gl.DYNAMIC_DRAW);
        this.capacity = batch.data.length;
      }
      gl.bufferSubData(gl.ARRAY_BUFFER, 0, batch.data, 0, batch.length);
    }
    // Rectangles, waves and hatches in the order they were put: a wave lies over the clip below it and under what came later.
    // Where a hatch and a wave were put after the same rectangles, the hatch goes first (no picture has both now).
    let from = 0;
    const waves = batch.waves;
    const hatches = batch.hatchData;
    let w = 0;
    let h = 0;
    while (w < waves.length || h < batch.hatchLength) {
      const hatchAt = h < batch.hatchLength ? hatches[h]! : Infinity;
      const wave = waves[w];
      if (!wave || hatchAt <= wave.at) {
        this.rects(from, hatchAt);
        from = Math.max(from, hatchAt);
        this.drawHatch(hatches, h, width, height);
        h += FLOATS_PER_HATCH;
      } else {
        this.rects(from, wave.at);
        from = Math.max(from, wave.at);
        this.drawWave(wave, width, height);
        w += 1;
      }
    }
    this.rects(from, batch.length);
    if (batch.waves.length > 0 || this.textures.held().textures > 0) this.textures.endPicture();
  }

  private useRects(): void {
    if (this.using === "rects") return;
    this.using = "rects";
    this.gl.useProgram(this.program);
    this.gl.bindVertexArray(this.vao);
  }

  /** The hatch whose floats start at `i`. */
  private drawHatch(d: Float32Array, i: number, width: number, height: number): void {
    const gl = this.gl;
    const hatch = this.hatch;
    if (!hatch) return;
    const at = hatch.at;
    if (this.using !== "hatch") {
      this.using = "hatch";
      gl.useProgram(hatch.program);
      gl.bindVertexArray(hatch.vao);
    }
    gl.uniform2f(at.u_size, width, height);
    gl.uniform4f(at.u_rect, d[i + 1]!, d[i + 2]!, d[i + 3]!, d[i + 4]!);
    gl.uniform2i(at.u_hatch, d[i + 5]!, d[i + 6]!);
    gl.uniform3f(at.u_color, d[i + 7]!, d[i + 8]!, d[i + 9]!);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  /** The rectangles whose floats are `from .. to`. */
  private rects(from: number, to: number): void {
    if (to <= from) return;
    this.useRects();
    this.gl.drawArrays(this.gl.TRIANGLES, from / FLOATS_PER_CORNER, (to - from) / FLOATS_PER_CORNER);
  }

  private drawWave(w: PlacedWave, width: number, height: number): void {
    const gl = this.gl;
    const wave = this.wave;
    if (!wave) return;
    const at = wave.at;
    if (this.using !== "wave") {
      this.using = "wave";
      gl.useProgram(wave.program);
      gl.bindVertexArray(wave.vao);
    }
    gl.bindTexture(gl.TEXTURE_2D, this.textures.use(w.source));
    gl.uniform2f(at.u_size, width, height);
    gl.uniform4f(at.u_rect, w.x0, w.y0, w.x1, w.y1);
    gl.uniform1i(at.u_stereo, w.source.peaks.channels === 2 ? 1 : 0);
    gl.uniform3i(at.u_index, w.base, w.first, w.end);
    gl.uniform3f(at.u_map, w.mapX, w.frac, w.step);
    gl.uniform4f(at.u_bars, w.barsX, w.pitch, w.bar, w.minH);
    gl.uniform2f(at.u_y, w.top, w.bottom);
    gl.uniform1f(at.u_gain, w.gain);
    gl.uniform4f(at.u_fade, w.fadeX0, w.fadeIn, w.fadeX1, w.fadeOut);
    gl.uniform3f(at.u_color, w.color[0], w.color[1], w.color[2]);
    if (w.mid) gl.uniform4f(at.u_mid, w.mid[0], w.mid[1], w.mid[2], 1);
    else gl.uniform4f(at.u_mid, 0, 0, 0, 0);
    if (w.line) gl.uniform4f(at.u_line, w.line[0], w.line[1], w.line[2], 1);
    else gl.uniform4f(at.u_line, 0, 0, 0, 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
  }

  /** The peaks as textures: one channel is RG16I, two are RGBA16I (wave.ts). */
  private texOps(): TexOps<WebGLTexture> {
    const gl = this.gl;
    const format = (channels: number) => (channels === 2 ? gl.RGBA_INTEGER : gl.RG_INTEGER);
    return {
      make: () => {
        const tex = gl.createTexture()!;
        gl.bindTexture(gl.TEXTURE_2D, tex);
        // A whole-number texture is only complete without filtering.
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        return tex;
      },
      alloc: (tex, channels, rows) => {
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.texImage2D(gl.TEXTURE_2D, 0, channels === 2 ? gl.RGBA16I : gl.RG16I, WAVE_TEX_WIDTH, rows, 0, format(channels), gl.SHORT, null);
      },
      write: (tex, channels, to, values, offset) => {
        gl.bindTexture(gl.TEXTURE_2D, tex);
        gl.texSubImage2D(gl.TEXTURE_2D, 0, to.x, to.y, to.w, to.h, format(channels), gl.SHORT, values, offset);
      },
      free: (tex) => gl.deleteTexture(tex),
    };
  }
}
