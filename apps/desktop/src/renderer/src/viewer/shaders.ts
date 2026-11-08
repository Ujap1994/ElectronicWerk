/**
 * GLSL ES 3.00 shaders of the 2D engine.
 *
 * Every primitive is an instanced quad whose fragment shader computes the
 * signed distance to the exact shape; the coverage becomes the fragment
 * alpha, which multisampling turns into a sample mask (alpha-to-coverage).
 * Layer opacity comes from the constant blend colour, and a per-layer depth
 * value makes each sample blend at most once per layer, so overlapping
 * shapes of one layer never darken each other.
 */

const VERTEX_HEADER = `#version 300 es
precision highp float;
precision highp int;
precision highp usampler2D;
uniform vec2 u_center;
uniform vec2 u_scale;
uniform float u_px;
uniform float u_depth;
uniform usampler2D u_state;
flat out uint v_state;
uint stateOf(uint obj) {
  if (obj == 0xFFFFFFFFu) { return 0u; }
  return texelFetch(u_state, ivec2(int(obj & 2047u), int(obj >> 11u)), 0).r;
}
vec4 clipOf(vec2 p) {
  return vec4((p - u_center) * u_scale, u_depth, 1.0);
}
`;

const FRAGMENT_HEADER = `#version 300 es
precision highp float;
precision highp int;
uniform vec3 u_color;
uniform vec4 u_select;
uniform float u_hoverMix;
uniform float u_px;
flat in uint v_state;
out vec4 o_color;
vec3 tint(vec3 c) {
  if ((v_state & 1u) != 0u) { return mix(c, u_select.rgb, u_select.a); }
  if ((v_state & 2u) != 0u) { return mix(c, u_select.rgb, u_hoverMix); }
  return c;
}
void emit(float coverage) {
  if (coverage <= 0.004) { discard; }
  o_color = vec4(tint(u_color), coverage);
}
float cover(float dist) {
  return clamp(0.5 - dist * u_px, 0.0, 1.0);
}
`;

export const SEGMENT_VS = `${VERTEX_HEADER}
in vec2 a_corner;
in vec4 a_ab;
in float a_hw;
in uint a_obj;
out vec2 v_local;
flat out float v_len;
flat out float v_hw;
void main() {
  vec2 a = a_ab.xy;
  vec2 dir = a_ab.zw - a;
  float len = length(dir);
  vec2 d = len > 0.0 ? dir / len : vec2(1.0, 0.0);
  vec2 n = vec2(-d.y, d.x);
  float hw = max(a_hw, 0.5 / u_px);
  float ext = hw + 1.0 / u_px;
  float x = mix(-ext, len + ext, a_corner.x);
  float y = a_corner.y * ext;
  v_local = vec2(x, y);
  v_len = len;
  v_hw = hw;
  v_state = stateOf(a_obj);
  gl_Position = clipOf(a + d * x + n * y);
}`;

export const SEGMENT_FS = `${FRAGMENT_HEADER}
in vec2 v_local;
flat in float v_len;
flat in float v_hw;
void main() {
  float dx = max(max(-v_local.x, v_local.x - v_len), 0.0);
  emit(cover(length(vec2(dx, v_local.y)) - v_hw));
}`;

export const ARC_VS = `${VERTEX_HEADER}
in vec2 a_corner;
in vec2 a_c;
in vec4 a_arc;
in uint a_obj;
out vec2 v_p;
flat out vec4 v_arc;
void main() {
  float hw = max(a_arc.y, 0.5 / u_px);
  float ext = a_arc.x + hw + 1.0 / u_px;
  v_p = a_corner * ext;
  v_arc = vec4(a_arc.x, hw, a_arc.z, a_arc.w);
  v_state = stateOf(a_obj);
  gl_Position = clipOf(a_c + v_p);
}`;

export const ARC_FS = `${FRAGMENT_HEADER}
in vec2 v_p;
flat in vec4 v_arc;
void main() {
  float r = v_arc.x;
  float hw = v_arc.y;
  float start = v_arc.z;
  float sweep = v_arc.w;
  if (sweep < 0.0) { start += sweep; sweep = -sweep; }
  float rel = mod(atan(v_p.y, v_p.x) - start, 6.28318531);
  float dist;
  if (sweep >= 6.2831 || rel <= sweep) {
    dist = abs(length(v_p) - r) - hw;
  } else {
    vec2 e0 = r * vec2(cos(start), sin(start));
    vec2 e1 = r * vec2(cos(start + sweep), sin(start + sweep));
    dist = min(length(v_p - e0), length(v_p - e1)) - hw;
  }
  emit(cover(dist));
}`;

export const CIRCLE_VS = `${VERTEX_HEADER}
in vec2 a_corner;
in vec2 a_c;
in vec2 a_r;
in uint a_obj;
out vec2 v_p;
flat out vec2 v_r;
void main() {
  float ro = max(a_r.x, 0.5 / u_px);
  float ext = ro + 1.0 / u_px;
  v_p = a_corner * ext;
  v_r = vec2(ro, a_r.y);
  v_state = stateOf(a_obj);
  gl_Position = clipOf(a_c + v_p);
}`;

export const CIRCLE_FS = `${FRAGMENT_HEADER}
in vec2 v_p;
flat in vec2 v_r;
void main() {
  float d = length(v_p);
  emit(cover(max(d - v_r.x, v_r.y - d)));
}`;

export const RECT_VS = `${VERTEX_HEADER}
in vec2 a_corner;
in vec2 a_c;
in vec2 a_half;
in vec2 a_shape;
in uint a_obj;
out vec2 v_p;
flat out vec2 v_half;
flat out float v_round;
void main() {
  vec2 half_size = max(a_half, vec2(0.5 / u_px));
  vec2 local = a_corner * (half_size + vec2(1.0 / u_px));
  float c = cos(a_shape.x);
  float s = sin(a_shape.x);
  v_p = local;
  v_half = half_size;
  v_round = min(a_shape.y, min(half_size.x, half_size.y));
  v_state = stateOf(a_obj);
  gl_Position = clipOf(a_c + vec2(local.x * c - local.y * s, local.x * s + local.y * c));
}`;

export const RECT_FS = `${FRAGMENT_HEADER}
in vec2 v_p;
flat in vec2 v_half;
flat in float v_round;
void main() {
  vec2 q = abs(v_p) - (v_half - vec2(v_round));
  float dist = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - v_round;
  emit(cover(dist));
}`;

export const MESH_VS = `${VERTEX_HEADER}
in vec2 a_pos;
in uint a_obj;
in uint a_flags;
flat out uint v_flags;
void main() {
  v_flags = a_flags;
  v_state = stateOf(a_obj);
  gl_Position = clipOf(a_pos);
}`;

export const MESH_FS = `${FRAGMENT_HEADER}
flat in uint v_flags;
uniform float u_hatch;
void main() {
  if ((v_flags & 1u) != 0u) {
    float s = mod(gl_FragCoord.x + gl_FragCoord.y, u_hatch);
    if (s > u_hatch * 0.2) { discard; }
  }
  emit(1.0);
}`;

export const TEXT_VS = `${VERTEX_HEADER}
uniform float u_minText;
in vec2 a_corner;
in vec2 a_p0;
in vec2 a_ux;
in vec2 a_uy;
in vec4 a_uv;
in vec2 a_misc;
in uint a_obj;
out vec2 v_uv;
flat out float v_edge;
void main() {
  v_state = stateOf(a_obj);
  v_edge = a_misc.y;
  v_uv = vec2(mix(a_uv.x, a_uv.z, a_corner.x), mix(a_uv.w, a_uv.y, a_corner.y));
  if (a_misc.x * u_px < u_minText) {
    gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
    return;
  }
  gl_Position = clipOf(a_p0 + a_ux * a_corner.x + a_uy * a_corner.y);
}`;

export const TEXT_FS = `${FRAGMENT_HEADER}
uniform sampler2D u_atlas;
in vec2 v_uv;
flat in float v_edge;
void main() {
  float d = texture(u_atlas, v_uv).r;
  float w = max(fwidth(d) * 0.7, 0.0001);
  emit(smoothstep(v_edge - w, v_edge + w, d));
}`;

export const GRID_VS = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

export const GRID_FS = `#version 300 es
precision highp float;
uniform vec2 u_phase;
uniform vec2 u_step;
uniform float u_dot;
uniform vec4 u_gridColor;
out vec4 o_color;
void main() {
  vec2 g = mod(gl_FragCoord.xy - u_phase, u_step);
  vec2 d = min(g, u_step - g);
  float a = 1.0 - smoothstep(u_dot - 0.5, u_dot + 0.5, length(d));
  if (a <= 0.0) { discard; }
  o_color = vec4(u_gridColor.rgb, u_gridColor.a * a);
}`;
