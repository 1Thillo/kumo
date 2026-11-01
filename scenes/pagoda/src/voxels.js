import * as THREE from 'three';

// ---------------------------------------------------------------- random & noise

export function mulberry32(seed) {
  let a = seed >>> 0;
  return function () {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hash3(x, y, z, seed) {
  let h = Math.imul(x, 374761393) ^ Math.imul(y, 668265263) ^ Math.imul(z, 1440670441) ^ Math.imul(seed + 1, 1274126177);
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  h ^= h >>> 16;
  return (h >>> 0) / 4294967296;
}

const fade = (t) => t * t * (3 - 2 * t);

export function noise2(x, z, seed = 0) {
  const ix = Math.floor(x), iz = Math.floor(z);
  const sx = fade(x - ix), sz = fade(z - iz);
  const a = hash3(ix, 0, iz, seed), b = hash3(ix + 1, 0, iz, seed);
  const c = hash3(ix, 0, iz + 1, seed), d = hash3(ix + 1, 0, iz + 1, seed);
  return a + (b - a) * sx + (c - a) * sz + (a - b - c + d) * sx * sz;
}

export function noise3(x, y, z, seed = 0) {
  const ix = Math.floor(x), iy = Math.floor(y), iz = Math.floor(z);
  const fx = fade(x - ix), fy = fade(y - iy), fz = fade(z - iz);
  const l = (a, b, t) => a + (b - a) * t;
  const h = (dx, dy, dz) => hash3(ix + dx, iy + dy, iz + dz, seed);
  return l(
    l(l(h(0, 0, 0), h(1, 0, 0), fx), l(h(0, 1, 0), h(1, 1, 0), fx), fy),
    l(l(h(0, 0, 1), h(1, 0, 1), fx), l(h(0, 1, 1), h(1, 1, 1), fx), fy),
    fz,
  );
}

export function fbm2(x, z, octaves = 3, seed = 0) {
  let sum = 0, amp = 0.5, freq = 1, norm = 0;
  for (let i = 0; i < octaves; i++) {
    sum += amp * noise2(x * freq, z * freq, seed + i * 17);
    norm += amp;
    amp *= 0.5;
    freq *= 2;
  }
  return sum / norm;
}

// ---------------------------------------------------------------- colour helpers (sRGB hex)

export function mixHex(a, b, t) {
  const ar = (a >> 16) & 255, ag = (a >> 8) & 255, ab = a & 255;
  const br = (b >> 16) & 255, bg = (b >> 8) & 255, bb = b & 255;
  const r = Math.round(ar + (br - ar) * t), g = Math.round(ag + (bg - ag) * t), bl = Math.round(ab + (bb - ab) * t);
  return (r << 16) | (g << 8) | bl;
}

export function shadeHex(c, f) {
  const r = Math.min(255, Math.max(0, Math.round(((c >> 16) & 255) * f)));
  const g = Math.min(255, Math.max(0, Math.round(((c >> 8) & 255) * f)));
  const b = Math.min(255, Math.max(0, Math.round((c & 255) * f)));
  return (r << 16) | (g << 8) | b;
}

// ---------------------------------------------------------------- grid

export const KIND_SOLID = 1;
export const KIND_GLOW = 2;

export class VoxelGrid {
  constructor(sx, sy, sz, ox, oy, oz, seed = 1) {
    this.sx = sx; this.sy = sy; this.sz = sz;
    this.ox = ox; this.oy = oy; this.oz = oz;
    const n = sx * sy * sz;
    this.col = new Int32Array(n);
    this.kind = new Uint8Array(n);
    this.sway = new Uint8Array(n);
    this.rand = mulberry32(seed);
  }

  idx(x, y, z) {
    const gx = Math.floor(x) + this.ox, gy = Math.floor(y) + this.oy, gz = Math.floor(z) + this.oz;
    if (gx < 0 || gy < 0 || gz < 0 || gx >= this.sx || gy >= this.sy || gz >= this.sz) return -1;
    return (gy * this.sz + gz) * this.sx + gx;
  }

  set(x, y, z, color, sway = 0, kind = KIND_SOLID, jitter = 0.05) {
    const i = this.idx(x, y, z);
    if (i < 0) return;
    if (jitter) color = shadeHex(color, 1 + (this.rand() - 0.5) * 2 * jitter);
    this.col[i] = color;
    this.kind[i] = kind;
    this.sway[i] = Math.round(Math.min(1, Math.max(0, sway)) * 255);
  }

  setIfEmpty(x, y, z, color, sway, kind, jitter) {
    if (!this.get(x, y, z)) this.set(x, y, z, color, sway, kind, jitter);
  }

  recolor(x, y, z, color, jitter = 0.05) {
    const i = this.idx(x, y, z);
    if (i < 0 || !this.kind[i]) return;
    this.col[i] = jitter ? shadeHex(color, 1 + (this.rand() - 0.5) * 2 * jitter) : color;
  }

  clear(x, y, z) {
    const i = this.idx(x, y, z);
    if (i >= 0) this.kind[i] = 0;
  }

  get(x, y, z) {
    const i = this.idx(x, y, z);
    return i < 0 ? 0 : this.kind[i];
  }
}

// ---------------------------------------------------------------- mesher (face culling + baked AO)

const AO_CURVE = [0.5, 0.68, 0.85, 1.0];
const FACES = [];
for (let d = 0; d < 3; d++) {
  for (const s of [1, -1]) {
    FACES.push({
      d, s,
      u: (d + 1) % 3,
      v: (d + 2) % 3,
      corners: s > 0 ? [[0, 0], [1, 0], [1, 1], [0, 1]] : [[0, 0], [0, 1], [1, 1], [1, 0]],
    });
  }
}

const newBuffer = () => ({ pos: [], nor: [], col: [], sway: [], idx: [] });

// Returns [static, wind, glow] geometries (null where empty).
export function meshGrid(g) {
  const { sx, sy, sz, ox, oy, oz } = g;
  const kind = g.kind;
  const bufs = [newBuffer(), newBuffer(), newBuffer()];
  const linear = new Map();
  const tmp = new THREE.Color();
  const occ = (x, y, z) =>
    x < 0 || y < 0 || z < 0 || x >= sx || y >= sy || z >= sz ? 0 : kind[(y * sz + z) * sx + x] ? 1 : 0;

  const b = [0, 0, 0], p = [0, 0, 0], v = [0, 0, 0], ao = [0, 0, 0, 0];

  for (let y = 0; y < sy; y++) {
    for (let z = 0; z < sz; z++) {
      for (let x = 0; x < sx; x++) {
        const i = (y * sz + z) * sx + x;
        const k = kind[i];
        if (!k) continue;
        const buf = k === KIND_GLOW ? bufs[2] : g.sway[i] ? bufs[1] : bufs[0];
        let lin = linear.get(g.col[i]);
        if (!lin) {
          tmp.setHex(g.col[i]);
          lin = [tmp.r, tmp.g, tmp.b];
          linear.set(g.col[i], lin);
        }
        const sw = g.sway[i] / 255;

        for (const f of FACES) {
          b[0] = x; b[1] = y; b[2] = z;
          b[f.d] += f.s;
          if (occ(b[0], b[1], b[2])) continue;
          const base = buf.pos.length / 3;
          for (let q = 0; q < 4; q++) {
            const cu = f.corners[q][0], cv = f.corners[q][1];
            const du = cu ? 1 : -1, dv = cv ? 1 : -1;
            p[0] = b[0]; p[1] = b[1]; p[2] = b[2];
            p[f.u] += du;
            const s1 = occ(p[0], p[1], p[2]);
            p[f.v] += dv;
            const cc = occ(p[0], p[1], p[2]);
            p[f.u] -= du;
            const s2 = occ(p[0], p[1], p[2]);
            const a = s1 && s2 ? 0 : 3 - (s1 + s2 + cc);
            ao[q] = a;

            v[0] = x; v[1] = y; v[2] = z;
            v[f.d] += f.s > 0 ? 1 : 0;
            v[f.u] += cu;
            v[f.v] += cv;
            buf.pos.push(v[0] - ox, v[1] - oy, v[2] - oz);
            buf.nor.push(f.d === 0 ? f.s : 0, f.d === 1 ? f.s : 0, f.d === 2 ? f.s : 0);
            const l = AO_CURVE[a];
            buf.col.push(lin[0] * l, lin[1] * l, lin[2] * l);
            buf.sway.push(sw);
          }
          if (ao[0] + ao[2] > ao[1] + ao[3]) buf.idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
          else buf.idx.push(base + 1, base + 2, base + 3, base + 1, base + 3, base);
        }
      }
    }
  }
  return bufs.map(toGeometry);
}

function toGeometry(buf) {
  if (!buf.idx.length) return null;
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(buf.pos, 3));
  geo.setAttribute('normal', new THREE.Float32BufferAttribute(buf.nor, 3));
  geo.setAttribute('color', new THREE.Float32BufferAttribute(buf.col, 3));
  geo.setAttribute('aSway', new THREE.Float32BufferAttribute(buf.sway, 1));
  geo.setIndex(buf.idx);
  geo.computeBoundingSphere();
  return geo;
}

// ---------------------------------------------------------------- materials

const WIND_GLSL = /* glsl */ `
#include <begin_vertex>
{
  vec3 wp = position;
  float gust = 0.65 + 0.35 * sin(uTime * 0.31 + wp.x * 0.015 + wp.z * 0.01);
  float ph = uTime * 1.15 + wp.x * 0.13 + wp.z * 0.11;
  float s = aSway * uWind * gust;
  transformed.x += (sin(ph) * 0.15 + sin(uTime * 2.8 + wp.y * 0.5 + wp.z * 0.3) * 0.05) * s;
  transformed.z += (cos(ph * 0.9 + 1.3) * 0.11 + cos(uTime * 3.2 + wp.x * 0.45) * 0.04) * s;
  transformed.y += sin(uTime * 2.1 + wp.x * 0.6 + wp.z * 0.4) * 0.03 * s;
}
`;

export function createVoxelMaterials(U) {
  const stat = new THREE.MeshLambertMaterial({ vertexColors: true });

  const wind = new THREE.MeshLambertMaterial({ vertexColors: true });
  wind.onBeforeCompile = (shader) => {
    shader.uniforms.uTime = U.uTime;
    shader.uniforms.uWind = U.uWind;
    shader.vertexShader = shader.vertexShader
      .replace('#include <common>', '#include <common>\nattribute float aSway;\nuniform float uTime;\nuniform float uWind;')
      .replace('#include <begin_vertex>', WIND_GLSL);
  };

  const glow = new THREE.MeshLambertMaterial({ vertexColors: true });
  glow.onBeforeCompile = (shader) => {
    shader.uniforms.uGlow = U.uGlow;
    shader.uniforms.uGlowDiffuse = U.uGlowDiffuse;
    shader.fragmentShader = shader.fragmentShader
      .replace('#include <common>', '#include <common>\nuniform float uGlow;\nuniform float uGlowDiffuse;')
      .replace(
        '#include <emissivemap_fragment>',
        '#include <emissivemap_fragment>\ntotalEmissiveRadiance += vColor.rgb * uGlow;\ndiffuseColor.rgb *= uGlowDiffuse;',
      );
  };

  return { stat, wind, glow };
}

// Meshes a grid into a group of up to three meshes.
export function voxelObject(grid, mats, { cast = true, receive = true } = {}) {
  const group = new THREE.Group();
  const [s, w, gl] = meshGrid(grid);
  for (const [geo, mat] of [[s, mats.stat], [w, mats.wind], [gl, mats.glow]]) {
    if (!geo) continue;
    const m = new THREE.Mesh(geo, mat);
    m.castShadow = cast;
    m.receiveShadow = receive;
    group.add(m);
  }
  return group;
}
