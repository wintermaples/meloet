import { FLOATS_PER_RECT, type Batch } from "./batch";

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

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type)!;
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(`gl: shader: ${gl.getShaderInfoLog(shader)}`);
  }
  return shader;
}

/** Draws a Batch: one buffer upload and one draw call per frame. */
export class Renderer {
  private readonly gl: WebGL2RenderingContext;
  private size: WebGLUniformLocation | null = null;
  private capacity = 0;

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext("webgl2", {
      alpha: true,
      antialias: false,
      depth: false,
      stencil: false,
      // WebKitGTK's snapshot (MELOET_SNAPSHOT) reads the canvas after the frame was shown;
      // without this it finds the buffer already cleared and the PNG lacks the whole surface.
      preserveDrawingBuffer: true,
    });
    if (!gl) throw new Error("gl: WebGL2 is not available");
    this.gl = gl;
    this.setUp();
  }

  /** Also called after the context comes back from a loss. */
  setUp(): void {
    const gl = this.gl;
    const program = gl.createProgram()!;
    gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX));
    gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      throw new Error(`gl: program: ${gl.getProgramInfoLog(program)}`);
    }
    gl.useProgram(program);
    this.size = gl.getUniformLocation(program, "u_size");

    gl.bindVertexArray(gl.createVertexArray());
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

  draw(batch: Batch, width: number, height: number): void {
    const gl = this.gl;
    gl.viewport(0, 0, width, height);
    gl.uniform2f(this.size, width, height);
    gl.clear(gl.COLOR_BUFFER_BIT);
    if (batch.length === 0) return;
    if (batch.data.length > this.capacity) {
      gl.bufferData(gl.ARRAY_BUFFER, batch.data.byteLength, gl.DYNAMIC_DRAW);
      this.capacity = batch.data.length;
    }
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, batch.data, 0, batch.length);
    gl.drawArrays(gl.TRIANGLES, 0, (batch.length / FLOATS_PER_RECT) * 6);
  }
}
