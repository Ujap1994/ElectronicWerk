/**
 * The WebGL2 renderer: uploads a scene once and draws it for a camera.
 */
import type { Camera } from './camera';
import { visibleRuns } from './culling';
import { GlyphAtlas } from './glyph-atlas';
import { rgba, type Palette } from './layers';
import type { Scene } from './scene';
import * as shaders from './shaders';
import { GLYPH_STRIDE, GlyphBuffer, layoutText } from './text-layout';
import type { Box, Section } from './types';

/** How one layer is drawn. */
export interface LayerAppearance {
  readonly visible: boolean;
  readonly opacity: number;
  readonly color: readonly [number, number, number];
}

export interface FrameParams {
  readonly camera: Camera;
  readonly dpr: number;
  /** Layer indices, back to front. */
  readonly order: readonly number[];
  readonly appearance: readonly LayerAppearance[];
  readonly palette: Palette;
  /** Grid spacing in scene units (0 = no grid). */
  readonly gridStep: number;
  /** A grid point (scene units); the grid runs through it. */
  readonly gridOrigin?: readonly [number, number];
  /** Offset of objects being moved (scene units). */
  readonly offset?: readonly [number, number];
}

interface Program {
  readonly program: WebGLProgram;
  readonly vao: WebGLVertexArrayObject;
  readonly uniforms: ReadonlyMap<string, WebGLUniformLocation>;
  readonly attributes: ReadonlyMap<string, number>;
}

type AttributeSpec = readonly [name: string, size: number, type: 'f' | 'u', offset: number];

const INSTANCE_LAYOUTS = {
  segments: {
    stride: 24,
    attributes: [
      ['a_ab', 4, 'f', 0],
      ['a_hw', 1, 'f', 16],
      ['a_obj', 1, 'u', 20],
    ],
  },
  arcs: {
    stride: 28,
    attributes: [
      ['a_c', 2, 'f', 0],
      ['a_arc', 4, 'f', 8],
      ['a_obj', 1, 'u', 24],
    ],
  },
  circles: {
    stride: 20,
    attributes: [
      ['a_c', 2, 'f', 0],
      ['a_r', 2, 'f', 8],
      ['a_obj', 1, 'u', 16],
    ],
  },
  rects: {
    stride: 28,
    attributes: [
      ['a_c', 2, 'f', 0],
      ['a_half', 2, 'f', 8],
      ['a_shape', 2, 'f', 16],
      ['a_obj', 1, 'u', 24],
    ],
  },
  glyphs: {
    stride: GLYPH_STRIDE * 4,
    attributes: [
      ['a_p0', 2, 'f', 0],
      ['a_ux', 2, 'f', 8],
      ['a_uy', 2, 'f', 16],
      ['a_uv', 4, 'f', 24],
      ['a_misc', 2, 'f', 40],
      ['a_obj', 1, 'u', 48],
    ],
  },
} as const satisfies Record<string, { stride: number; attributes: readonly AttributeSpec[] }>;

const MESH_ATTRIBUTES: readonly AttributeSpec[] = [
  ['a_pos', 2, 'f', 0],
  ['a_obj', 1, 'u', 8],
  ['a_flags', 1, 'u', 12],
];

type InstanceKind = 'segments' | 'arcs' | 'circles' | 'rects';
const INSTANCE_KINDS: readonly InstanceKind[] = ['rects', 'circles', 'segments', 'arcs'];

const STATE_WIDTH = 2048;
const MIN_TEXT_PX = 3.5;

export class WebGlUnavailableError extends Error {
  constructor() {
    super('WebGL 2 is not available');
    this.name = 'WebGlUnavailableError';
  }
}

export class SceneRenderer {
  readonly gl: WebGL2RenderingContext;
  readonly samples: number;
  private readonly programs: Readonly<
    Record<'segments' | 'arcs' | 'circles' | 'rects' | 'mesh' | 'text' | 'grid', Program>
  >;
  private readonly quads: Readonly<Record<'line' | 'centered' | 'unit', WebGLBuffer>>;
  private sceneBuffer: WebGLBuffer | null = null;
  private indexBuffer: WebGLBuffer | null = null;
  private glyphBuffer: WebGLBuffer | null = null;
  private stateTexture: WebGLTexture | null = null;
  private readonly atlasTexture: WebGLTexture;
  private atlas: GlyphAtlas | null = null;
  private scene: Scene | null = null;
  private indexOffsets: number[] = [];
  private glyphRanges: [number, number][] = [];

  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', {
      antialias: true,
      alpha: false,
      depth: true,
      stencil: false,
      premultipliedAlpha: false,
      powerPreference: 'high-performance',
    });
    if (gl === null) {
      throw new WebGlUnavailableError();
    }
    this.gl = gl;
    this.samples = gl.getParameter(gl.SAMPLES) as number;
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    this.quads = {
      line: this.staticBuffer([0, -1, 1, -1, 0, 1, 1, 1]),
      centered: this.staticBuffer([-1, -1, 1, -1, -1, 1, 1, 1]),
      unit: this.staticBuffer([0, 0, 1, 0, 0, 1, 1, 1]),
    };
    this.programs = {
      segments: this.program(shaders.SEGMENT_VS, shaders.SEGMENT_FS, 'line'),
      arcs: this.program(shaders.ARC_VS, shaders.ARC_FS, 'centered'),
      circles: this.program(shaders.CIRCLE_VS, shaders.CIRCLE_FS, 'centered'),
      rects: this.program(shaders.RECT_VS, shaders.RECT_FS, 'centered'),
      mesh: this.program(shaders.MESH_VS, shaders.MESH_FS, null),
      text: this.program(shaders.TEXT_VS, shaders.TEXT_FS, 'unit'),
      grid: this.program(shaders.GRID_VS, shaders.GRID_FS, null),
    };
    this.atlasTexture = this.createTexture();
  }

  /** Renderer description, for diagnostics and benchmarks. */
  get description(): string {
    const gl = this.gl;
    const info = gl.getExtension('WEBGL_debug_renderer_info');
    const renderer: unknown =
      info === null ? gl.getParameter(gl.RENDERER) : gl.getParameter(info.UNMASKED_RENDERER_WEBGL);
    return typeof renderer === 'string' ? renderer : 'unknown';
  }

  private staticBuffer(values: readonly number[]): WebGLBuffer {
    const gl = this.gl;
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(values), gl.STATIC_DRAW);
    return buffer;
  }

  private createTexture(): WebGLTexture {
    const gl = this.gl;
    const texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    return texture;
  }

  private compile(type: number, source: string): WebGLShader {
    const gl = this.gl;
    const shader = gl.createShader(type);
    if (shader === null) {
      throw new Error('cannot create shader');
    }
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (gl.getShaderParameter(shader, gl.COMPILE_STATUS) !== true) {
      throw new Error(`shader: ${gl.getShaderInfoLog(shader) ?? ''}`);
    }
    return shader;
  }

  private program(vs: string, fs: string, quad: 'line' | 'centered' | 'unit' | null): Program {
    const gl = this.gl;
    const program = gl.createProgram();
    gl.attachShader(program, this.compile(gl.VERTEX_SHADER, vs));
    gl.attachShader(program, this.compile(gl.FRAGMENT_SHADER, fs));
    gl.linkProgram(program);
    if (gl.getProgramParameter(program, gl.LINK_STATUS) !== true) {
      throw new Error(`program: ${gl.getProgramInfoLog(program) ?? ''}`);
    }
    const uniforms = new Map<string, WebGLUniformLocation>();
    const uniformCount = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS) as number;
    for (let i = 0; i < uniformCount; i += 1) {
      const info = gl.getActiveUniform(program, i);
      const location = info === null ? null : gl.getUniformLocation(program, info.name);
      if (info !== null && location !== null) {
        uniforms.set(info.name, location);
      }
    }
    const attributes = new Map<string, number>();
    const attributeCount = gl.getProgramParameter(program, gl.ACTIVE_ATTRIBUTES) as number;
    for (let i = 0; i < attributeCount; i += 1) {
      const info = gl.getActiveAttrib(program, i);
      if (info !== null) {
        attributes.set(info.name, gl.getAttribLocation(program, info.name));
      }
    }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const corner = attributes.get('a_corner');
    if (quad !== null && corner !== undefined) {
      gl.bindBuffer(gl.ARRAY_BUFFER, this.quads[quad]);
      gl.enableVertexAttribArray(corner);
      gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);
    }
    gl.bindVertexArray(null);
    return { program, vao, uniforms, attributes };
  }

  private pointers(
    program: Program,
    specs: readonly AttributeSpec[],
    stride: number,
    base: number,
    divisor: number,
  ): void {
    const gl = this.gl;
    for (const [name, size, type, offset] of specs) {
      const location = program.attributes.get(name);
      if (location === undefined || location < 0) {
        continue;
      }
      gl.enableVertexAttribArray(location);
      if (type === 'u') {
        gl.vertexAttribIPointer(location, size, gl.UNSIGNED_INT, stride, base + offset);
      } else {
        gl.vertexAttribPointer(location, size, gl.FLOAT, false, stride, base + offset);
      }
      gl.vertexAttribDivisor(location, divisor);
    }
  }

  /** Uploads a scene (the canvas font must be loaded). */
  setScene(scene: Scene | null): void {
    const gl = this.gl;
    this.scene = scene;
    for (const buffer of [this.sceneBuffer, this.indexBuffer, this.glyphBuffer]) {
      if (buffer !== null) {
        gl.deleteBuffer(buffer);
      }
    }
    this.sceneBuffer = null;
    this.indexBuffer = null;
    this.glyphBuffer = null;
    if (scene === null) {
      return;
    }
    gl.bindVertexArray(null);
    this.sceneBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.sceneBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Uint8Array(scene.buffer), gl.STATIC_DRAW);

    // Indices of all layer meshes in one buffer (WebGL forbids sharing a
    // buffer between vertex and index data).
    const total = scene.layers.reduce((sum, layer) => sum + layer.indices.length, 0);
    const indices = new Uint32Array(Math.max(total, 1));
    this.indexOffsets = [];
    let at = 0;
    for (const layer of scene.layers) {
      this.indexOffsets.push(at * 4);
      indices.set(layer.indices, at);
      at += layer.indices.length;
    }
    this.indexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.indexBuffer);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);

    // Glyphs.
    const atlas = (this.atlas ??= new GlyphAtlas());
    const glyphs = new GlyphBuffer();
    this.glyphRanges = scene.layers.map((layer) => {
      const start = glyphs.count;
      for (const text of layer.meta.texts) {
        layoutText(text, scene.font, atlas, glyphs);
      }
      return [start, glyphs.count - start];
    });
    this.glyphBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphBuffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      glyphs.count > 0 ? glyphs.data() : new Float32Array(GLYPH_STRIDE),
      gl.STATIC_DRAW,
    );
    this.uploadAtlas(true);
    this.setState(new Uint8Array(scene.objectCount));
  }

  private uploadAtlas(full: boolean): void {
    const atlas = this.atlas;
    if (atlas === null || (!full && atlas.dirty === null)) {
      return;
    }
    const gl = this.gl;
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.atlasTexture);
    if (full) {
      gl.texImage2D(
        gl.TEXTURE_2D,
        0,
        gl.R8,
        atlas.size,
        atlas.size,
        0,
        gl.RED,
        gl.UNSIGNED_BYTE,
        atlas.pixels,
      );
    } else if (atlas.dirty !== null) {
      const { y0, y1 } = atlas.dirty;
      gl.texSubImage2D(
        gl.TEXTURE_2D,
        0,
        0,
        y0,
        atlas.size,
        y1 - y0,
        gl.RED,
        gl.UNSIGNED_BYTE,
        atlas.pixels.subarray(y0 * atlas.size, y1 * atlas.size),
      );
    }
    atlas.dirty = null;
  }

  /**
   * Per-object flags (1 selected, 2 hovered, 4 moving, 8 highlighted net),
   * one byte per object.
   */
  setState(flags: Uint8Array): void {
    const gl = this.gl;
    const height = Math.max(1, Math.ceil(flags.length / STATE_WIDTH));
    const data = new Uint8Array(STATE_WIDTH * height);
    data.set(flags);
    this.stateTexture ??= gl.createTexture();
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.stateTexture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.R8UI,
      STATE_WIDTH,
      height,
      0,
      gl.RED_INTEGER,
      gl.UNSIGNED_BYTE,
      data,
    );
  }

  private use(program: Program, frame: FrameParams, depth: number): void {
    const gl = this.gl;
    const { camera, dpr, palette } = frame;
    gl.useProgram(program.program);
    gl.bindVertexArray(program.vao);
    const u = (name: string): WebGLUniformLocation | null => program.uniforms.get(name) ?? null;
    const sign = camera.flipped ? -1 : 1;
    gl.uniform2f(u('u_center'), camera.cx, camera.cy);
    gl.uniform2f(
      u('u_scale'),
      (2 * camera.scale * sign) / camera.width,
      (2 * camera.scale) / camera.height,
    );
    gl.uniform1f(u('u_px'), camera.scale * dpr);
    gl.uniform1f(u('u_depth'), depth);
    gl.uniform1i(u('u_state'), 1);
    gl.uniform1i(u('u_atlas'), 0);
    gl.uniform1f(u('u_minText'), MIN_TEXT_PX * dpr);
    gl.uniform1f(u('u_hatch'), 8 * dpr);
    const select = rgba(palette.select);
    gl.uniform4f(u('u_select'), select[0], select[1], select[2], palette.selectMix);
    const highlight = rgba(palette.highlight);
    gl.uniform4f(u('u_highlight'), highlight[0], highlight[1], highlight[2], palette.highlightMix);
    gl.uniform1f(u('u_hoverMix'), palette.hoverMix);
    const offset = frame.offset ?? [0, 0];
    gl.uniform2f(u('u_offset'), offset[0], offset[1]);
  }

  private drawGrid(frame: FrameParams): void {
    const { camera, dpr, gridStep } = frame;
    if (gridStep <= 0) {
      return;
    }
    const gl = this.gl;
    const program = this.programs.grid;
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.SAMPLE_ALPHA_TO_COVERAGE);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    gl.useProgram(program.program);
    gl.bindVertexArray(program.vao);
    const u = (name: string): WebGLUniformLocation | null => program.uniforms.get(name) ?? null;
    // A grid line near the centre, in device pixels (bottom-up).
    const [ox, oy] = frame.gridOrigin ?? [0, 0];
    const gx = ox + Math.round((camera.cx - ox) / gridStep) * gridStep;
    const gy = oy + Math.round((camera.cy - oy) / gridStep) * gridStep;
    const [sx, sy] = camera.toScreen(gx, gy);
    const step = gridStep * camera.scale * dpr;
    gl.uniform2f(u('u_phase'), sx * dpr + 0.5, (camera.height - sy) * dpr + 0.5);
    gl.uniform2f(u('u_step'), step, step);
    gl.uniform1f(u('u_dot'), Math.max(0.8, dpr * 0.75));
    const color = rgba(frame.palette.grid);
    gl.uniform4f(u('u_gridColor'), color[0], color[1], color[2], color[3]);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
  }

  private drawInstances(
    kind: InstanceKind,
    section: Section,
    frame: FrameParams,
    depth: number,
    view: Box,
    color: readonly [number, number, number],
  ): void {
    const scene = this.scene;
    if (scene === null || section.count === 0 || this.sceneBuffer === null) {
      return;
    }
    const gl = this.gl;
    const program = this.programs[kind];
    const layout = INSTANCE_LAYOUTS[kind];
    this.use(program, frame, depth);
    gl.uniform3f(program.uniforms.get('u_color') ?? null, color[0], color[1], color[2]);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.sceneBuffer);
    const chunkSize = scene.meta.chunk;
    const chunks = new Float32Array(
      scene.buffer,
      section.chunks,
      Math.ceil(section.count / chunkSize) * 4,
    );
    for (const [first, count] of visibleRuns(chunks, section.count, chunkSize, view)) {
      this.pointers(
        program,
        layout.attributes,
        layout.stride,
        section.offset + first * layout.stride,
        1,
      );
      gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, count);
    }
  }

  /** Draws one frame. */
  render(frame: FrameParams): void {
    const gl = this.gl;
    const { camera, dpr } = frame;
    const width = Math.round(camera.width * dpr);
    const height = Math.round(camera.height * dpr);
    gl.viewport(0, 0, width, height);
    const background = rgba(frame.palette.background);
    gl.clearColor(background[0], background[1], background[2], 1);
    gl.clearDepth(1);
    gl.depthMask(true);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    const scene = this.scene;
    if (scene === null) {
      this.drawGrid(frame);
      return;
    }
    this.uploadAtlas(false);
    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, this.stateTexture);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.atlasTexture);

    // Margin so that wide shapes at the border are not culled.
    const visible = camera.visible();
    const pad = 4 / camera.scale;
    const view: Box = [visible[0] - pad, visible[1] - pad, visible[2] + pad, visible[3] + pad];
    const order = frame.order;
    const gridAfter = scene.layers[order[0] ?? -1]?.meta.key === 'sch.page' ? 0 : -1;
    if (gridAfter < 0) {
      this.drawGrid(frame);
    }
    const n = order.length;
    order.forEach((index, position) => {
      const layer = scene.layers[index];
      const look = frame.appearance[index];
      if (layer !== undefined && look?.visible === true && look.opacity > 0) {
        const depth = 0.9 - (1.8 * (position + 1)) / (n + 1);
        gl.enable(gl.DEPTH_TEST);
        gl.depthFunc(gl.LESS);
        gl.depthMask(true);
        gl.enable(gl.BLEND);
        gl.blendFunc(gl.CONSTANT_ALPHA, gl.ONE_MINUS_CONSTANT_ALPHA);
        gl.blendColor(0, 0, 0, look.opacity);
        gl.enable(gl.SAMPLE_ALPHA_TO_COVERAGE);
        this.drawLayer(index, layer.meta.mesh.indexCount, frame, depth, view, look.color);
        for (const kind of INSTANCE_KINDS) {
          this.drawInstances(kind, layer.meta[kind], frame, depth, view, look.color);
        }
        this.drawGlyphs(index, frame, depth, look.color);
      }
      if (position === gridAfter) {
        this.drawGrid(frame);
      }
    });
    gl.disable(gl.SAMPLE_ALPHA_TO_COVERAGE);
    gl.bindVertexArray(null);
  }

  private drawLayer(
    index: number,
    indexCount: number,
    frame: FrameParams,
    depth: number,
    _view: Box,
    color: readonly [number, number, number],
  ): void {
    const scene = this.scene;
    const layer = scene?.layers[index];
    if (indexCount === 0 || layer === undefined || this.sceneBuffer === null) {
      return;
    }
    const gl = this.gl;
    const program = this.programs.mesh;
    this.use(program, frame, depth);
    gl.uniform3f(program.uniforms.get('u_color') ?? null, color[0], color[1], color[2]);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.sceneBuffer);
    this.pointers(program, MESH_ATTRIBUTES, 16, layer.meta.mesh.vertices, 0);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.indexBuffer);
    gl.drawElements(gl.TRIANGLES, indexCount, gl.UNSIGNED_INT, this.indexOffsets[index] ?? 0);
  }

  private drawGlyphs(
    index: number,
    frame: FrameParams,
    depth: number,
    color: readonly [number, number, number],
  ): void {
    const range = this.glyphRanges[index];
    if (range === undefined || range[1] === 0 || this.glyphBuffer === null) {
      return;
    }
    const gl = this.gl;
    const program = this.programs.text;
    const layout = INSTANCE_LAYOUTS.glyphs;
    this.use(program, frame, depth);
    gl.uniform3f(program.uniforms.get('u_color') ?? null, color[0], color[1], color[2]);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.glyphBuffer);
    this.pointers(program, layout.attributes, layout.stride, range[0] * layout.stride, 1);
    gl.drawArraysInstanced(gl.TRIANGLE_STRIP, 0, 4, range[1]);
  }

  dispose(): void {
    this.setScene(null);
    const gl = this.gl;
    gl.deleteTexture(this.atlasTexture);
    if (this.stateTexture !== null) {
      gl.deleteTexture(this.stateTexture);
    }
    for (const program of Object.values(this.programs)) {
      gl.deleteProgram(program.program);
      gl.deleteVertexArray(program.vao);
    }
    for (const buffer of Object.values(this.quads)) {
      gl.deleteBuffer(buffer);
    }
  }
}
