import * as THREE from 'three';
import { VoxelGrid, meshGrid, mulberry32 } from './voxels.js';
import { POND, WATER_Y, SURF } from './garden.js';

const rand = mulberry32(99);

// ---------------------------------------------------------------- soft round sprite texture

export function makeGlowTexture() {
  const c = document.createElement('canvas');
  c.width = c.height = 128;
  const ctx = c.getContext('2d');
  const g = ctx.createRadialGradient(64, 64, 0, 64, 64, 64);
  g.addColorStop(0, 'rgba(255,255,255,1)');
  g.addColorStop(0.18, 'rgba(255,255,255,0.55)');
  g.addColorStop(0.45, 'rgba(255,255,255,0.14)');
  g.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, 128, 128);
  const tex = new THREE.CanvasTexture(c);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

// ---------------------------------------------------------------- falling sakura petals

export class Petals {
  constructor(scene, spawns, groundAt, max = 1000) {
    this.spawns = spawns;
    this.groundAt = groundAt;
    this.max = max;
    this.count = 520;
    const geo = new THREE.PlaneGeometry(0.36, 0.28);
    const mat = new THREE.MeshLambertMaterial({ color: 0xffffff, side: THREE.DoubleSide, emissive: 0x2a1018 });
    this.mesh = new THREE.InstancedMesh(geo, mat, max);
    this.mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.mesh.frustumCulled = false;
    const pinks = [0xffd6e4, 0xf9b9d0, 0xffe8f0, 0xf5a6c4, 0xfcc8da];
    const c = new THREE.Color();
    for (let i = 0; i < max; i++) this.mesh.setColorAt(i, c.setHex(pinks[i % pinks.length]));
    this.p = new Float32Array(max * 3);
    this.r = new Float32Array(max * 3);
    this.spin = new Float32Array(max * 3);
    this.phase = new Float32Array(max);
    this.speed = new Float32Array(max);
    this.landed = new Float32Array(max);
    for (let i = 0; i < max; i++) this.respawn(i, true);
    this.dummy = new THREE.Object3D();
    scene.add(this.mesh);
  }

  respawn(i, initial) {
    const s = this.spawns[Math.floor(rand() * this.spawns.length)];
    const p = this.p, k = i * 3;
    p[k] = s[0] + (rand() - 0.5);
    p[k + 1] = s[1] - (initial ? rand() * 10 : 0);
    p[k + 2] = s[2] + (rand() - 0.5);
    if (initial) {
      p[k] += (rand() - 0.5) * 14;
      p[k + 2] += (rand() - 0.5) * 14;
    }
    this.r[k] = rand() * 6.3; this.r[k + 1] = rand() * 6.3; this.r[k + 2] = rand() * 6.3;
    this.spin[k] = (rand() - 0.5) * 5; this.spin[k + 1] = (rand() - 0.5) * 3; this.spin[k + 2] = (rand() - 0.5) * 5;
    this.phase[i] = rand() * 100;
    this.speed[i] = 0.45 + rand() * 0.45;
    this.landed[i] = 0;
  }

  update(dt, t, wind) {
    const { p, r, spin, phase, speed, landed, dummy } = this;
    const n = Math.min(this.max, Math.round(this.count));
    for (let i = 0; i < n; i++) {
      const k = i * 3;
      let scale = 1;
      if (landed[i] > 0) {
        landed[i] -= dt;
        scale = Math.min(1, landed[i] / 1.2);
        if (landed[i] <= 0) { this.respawn(i, false); scale = 0; }
      } else {
        const ph = phase[i];
        p[k] += (wind.x * 1.1 + Math.sin(t * 1.7 + ph * 3) * 0.45) * dt;
        p[k + 2] += (wind.z * 1.1 + Math.cos(t * 1.3 + ph * 2) * 0.45) * dt;
        p[k + 1] -= (speed[i] + Math.sin(t * 2.6 + ph) * 0.25) * dt;
        r[k] += spin[k] * dt; r[k + 1] += spin[k + 1] * dt; r[k + 2] += spin[k + 2] * dt;
        const gy = this.groundAt(p[k], p[k + 2]);
        if (gy > -100 && p[k + 1] <= gy + 0.04 && p[k + 1] > gy - 1.5) {
          p[k + 1] = gy + 0.04;
          r[k] = -Math.PI / 2; r[k + 1] = 0;
          landed[i] = 3 + rand() * 6;
        } else if (p[k + 1] < -70) this.respawn(i, false);
      }
      dummy.position.set(p[k], p[k + 1], p[k + 2]);
      dummy.rotation.set(r[k], r[k + 1], r[k + 2]);
      dummy.scale.setScalar(scale);
      dummy.updateMatrix();
      this.mesh.setMatrixAt(i, dummy.matrix);
    }
    this.mesh.count = n;
    this.mesh.instanceMatrix.needsUpdate = true;
  }
}

// ---------------------------------------------------------------- pond surface

const WATER_VERT = /* glsl */ `
varying vec3 vW;
void main() {
  vec4 w = modelMatrix * vec4(position, 1.0);
  vW = w.xyz;
  gl_Position = projectionMatrix * viewMatrix * w;
}
`;

const WATER_FRAG = /* glsl */ `
uniform float uTime, uLight, uSpec;
uniform vec3 uDeep, uShallow, uSky, uSunDir, uSunCol, uGlowCol;
uniform float uGlow;
varying vec3 vW;
float wave(vec2 p) {
  return sin(p.x * 1.7 + uTime * 1.3) * 0.5 + sin(p.y * 2.3 - uTime * 1.1) * 0.5
       + sin((p.x - p.y) * 3.1 + uTime * 2.0) * 0.25 + sin((p.x * 0.7 + p.y * 1.3) * 5.3 - uTime * 2.6) * 0.12;
}
void main() {
  vec2 p = vW.xz;
  float e = 0.05;
  float h = wave(p);
  vec3 n = normalize(vec3(-(wave(p + vec2(e, 0.0)) - h) / e * 0.07, 1.0, -(wave(p + vec2(0.0, e)) - h) / e * 0.07));
  vec3 V = normalize(cameraPosition - vW);
  float fres = pow(1.0 - max(dot(n, V), 0.0), 3.0);
  vec3 base = mix(uShallow, uDeep, 0.5 + 0.5 * sin(p.x * 0.31) * sin(p.y * 0.37)) * uLight;
  vec3 col = mix(base, uSky, 0.22 + 0.55 * fres);
  vec3 R = reflect(-V, n);
  col += uSunCol * pow(max(dot(R, uSunDir), 0.0), 140.0) * uSpec;
  float c = smoothstep(0.7, 1.0, sin(p.x * 2.6 + h * 2.0 + uTime * 0.8) * sin(p.y * 2.2 - h * 1.5 + uTime * 0.6));
  col += c * 0.08 * uLight;
  col += uGlowCol * uGlow * (0.12 + 0.25 * c);
  gl_FragColor = vec4(col, clamp(0.66 + fres * 0.28, 0.0, 0.94));
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

export function createWater(cells) {
  const pos = [], idx = [];
  for (const [x, z] of cells) {
    const b = pos.length / 3;
    pos.push(x, WATER_Y, z, x, WATER_Y, z + 1, x + 1, WATER_Y, z + 1, x + 1, WATER_Y, z);
    idx.push(b, b + 1, b + 2, b, b + 2, b + 3);
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  geo.setIndex(idx);
  const uniforms = {
    uTime: { value: 0 }, uLight: { value: 1 }, uSpec: { value: 1 }, uGlow: { value: 0 },
    uDeep: { value: new THREE.Color(0x1f6f7a) }, uShallow: { value: new THREE.Color(0x4fb3b0) },
    uSky: { value: new THREE.Color() }, uSunDir: { value: new THREE.Vector3(0, 1, 0) },
    uSunCol: { value: new THREE.Color() }, uGlowCol: { value: new THREE.Color(0xffb35a) },
  };
  const mat = new THREE.ShaderMaterial({
    uniforms, vertexShader: WATER_VERT, fragmentShader: WATER_FRAG, transparent: true, depthWrite: false,
  });
  const mesh = new THREE.Mesh(geo, mat);
  mesh.renderOrder = 2;
  return { mesh, uniforms };
}

// ---------------------------------------------------------------- waterfall off the island edge

const FALL_VERT = /* glsl */ `
varying vec2 vUv;
void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }
`;
const FALL_FRAG = /* glsl */ `
uniform float uTime, uLight;
uniform vec3 uCol;
varying vec2 vUv;
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float vnoise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), f.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x), f.y);
}
void main() {
  float v = vUv.y;
  float s = vnoise(vec2(vUv.x * 6.0, v * 26.0 + uTime * 4.2)) * 0.6 + vnoise(vec2(vUv.x * 14.0, v * 60.0 + uTime * 7.0)) * 0.4;
  float edge = smoothstep(0.0, 0.2, vUv.x) * smoothstep(1.0, 0.8, vUv.x);
  float fade = smoothstep(0.0, 0.6, v);
  vec3 col = mix(uCol, vec3(1.0), smoothstep(0.42, 0.85, s)) * uLight;
  gl_FragColor = vec4(col, edge * fade * (0.5 + 0.5 * s));
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

const POINT_VERT = /* glsl */ `
attribute float aAlpha;
attribute float aSize;
uniform float uScale;
varying float vA;
void main() {
  vA = aAlpha;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  gl_Position = projectionMatrix * mv;
  gl_PointSize = aSize * uScale / -mv.z;
}
`;
const POINT_FRAG = /* glsl */ `
uniform vec3 uCol;
varying float vA;
void main() {
  float d = length(gl_PointCoord - 0.5);
  float a = smoothstep(0.5, 0.0, d) * vA;
  gl_FragColor = vec4(uCol, a);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

function pointSystem(count, color, additive) {
  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(count * 3), 3).setUsage(THREE.DynamicDrawUsage));
  geo.setAttribute('aAlpha', new THREE.BufferAttribute(new Float32Array(count), 1).setUsage(THREE.DynamicDrawUsage));
  geo.setAttribute('aSize', new THREE.BufferAttribute(new Float32Array(count), 1));
  const uniforms = { uScale: { value: 500 }, uCol: { value: new THREE.Color(color) } };
  const mat = new THREE.ShaderMaterial({
    uniforms, vertexShader: POINT_VERT, fragmentShader: POINT_FRAG, transparent: true, depthWrite: false,
    blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending,
  });
  const pts = new THREE.Points(geo, mat);
  pts.frustumCulled = false;
  return { pts, geo, uniforms };
}

export class Waterfall {
  constructor(scene, wf) {
    this.wf = wf;
    const segs = 48, drop = 52;
    const pos = [], uv = [], idx = [];
    for (let i = 0; i <= segs; i++) {
      const t = i / segs, d = t * drop;
      const x = wf.x + 1.7 * (1 - Math.exp(-d * 0.22)) + d * 0.02;
      const y = wf.y - d;
      const spread = d * 0.03;
      pos.push(x, y, wf.z0 - spread, x, y, wf.z1 + spread);
      uv.push(0, 1 - t, 1, 1 - t);
      if (i < segs) { const b = i * 2; idx.push(b, b + 1, b + 3, b, b + 3, b + 2); }
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
    geo.setAttribute('uv', new THREE.Float32BufferAttribute(uv, 2));
    geo.setIndex(idx);
    this.uniforms = { uTime: { value: 0 }, uLight: { value: 1 }, uCol: { value: new THREE.Color(0x8fd3e8) } };
    const mat = new THREE.ShaderMaterial({
      uniforms: this.uniforms, vertexShader: FALL_VERT, fragmentShader: FALL_FRAG,
      transparent: true, depthWrite: false, side: THREE.DoubleSide,
    });
    this.mesh = new THREE.Mesh(geo, mat);
    this.mesh.renderOrder = 3;
    scene.add(this.mesh);

    // spray / mist
    this.n = 160;
    const sys = pointSystem(this.n, 0xeaf6ff, false);
    this.spray = sys;
    this.d = new Float32Array(this.n);
    this.off = new Float32Array(this.n * 2);
    this.v = new Float32Array(this.n);
    const size = sys.geo.attributes.aSize.array;
    for (let i = 0; i < this.n; i++) { this.reset(i); this.d[i] = rand() * 40; size[i] = 0.5 + rand() * 1.4; }
    sys.pts.renderOrder = 4;
    scene.add(sys.pts);
  }

  reset(i) {
    this.d[i] = 0;
    this.off[i * 2] = (rand() - 0.5) * 1.2;
    this.off[i * 2 + 1] = rand();
    this.v[i] = 7 + rand() * 6;
  }

  update(dt, t, light) {
    this.uniforms.uTime.value = t;
    this.uniforms.uLight.value = light;
    const { wf } = this;
    const pos = this.spray.geo.attributes.position.array, al = this.spray.geo.attributes.aAlpha.array;
    for (let i = 0; i < this.n; i++) {
      this.d[i] += this.v[i] * dt;
      if (this.d[i] > 42) this.reset(i);
      const d = this.d[i];
      const out = 1.7 * (1 - Math.exp(-d * 0.22)) + d * 0.02 + this.off[i * 2] * (0.3 + d * 0.06);
      pos[i * 3] = wf.x + out + 0.3;
      pos[i * 3 + 1] = wf.y - d;
      pos[i * 3 + 2] = wf.z0 - d * 0.03 + this.off[i * 2 + 1] * (wf.z1 - wf.z0 + d * 0.06);
      al[i] = Math.min(1, d * 0.5) * Math.max(0, 1 - d / 42) * 0.5 * light;
    }
    this.spray.geo.attributes.position.needsUpdate = true;
    this.spray.geo.attributes.aAlpha.needsUpdate = true;
  }
}

// ---------------------------------------------------------------- fireflies (night only)

export class Fireflies {
  constructor(scene, info, count = 80) {
    this.n = count;
    const sys = pointSystem(count, 0xd9ff7a, true);
    this.sys = sys;
    this.base = new Float32Array(count * 3);
    this.ph = new Float32Array(count);
    const areas = [[POND.cx, POND.cz, 11], [-19, 15, 9], [0, 18, 8], [-23, -14, 8], [22, -19, 8], [27, 24, 7]];
    const size = sys.geo.attributes.aSize.array;
    for (let i = 0; i < count; i++) {
      const [cx, cz, r] = areas[i % areas.length];
      const a = rand() * Math.PI * 2, d = Math.sqrt(rand()) * r;
      const x = cx + Math.cos(a) * d, z = cz + Math.sin(a) * d;
      const ci = info.ci(x, z);
      const g = ci >= 0 && info.H[ci] > -100 ? Math.max(1, info.H[ci] + 1) : 1;
      this.base[i * 3] = x; this.base[i * 3 + 1] = g + 1 + rand() * 4; this.base[i * 3 + 2] = z;
      this.ph[i] = rand() * 100;
      size[i] = 0.35 + rand() * 0.3;
    }
    sys.pts.renderOrder = 5;
    scene.add(sys.pts);
  }

  update(t, vis) {
    this.sys.pts.visible = vis > 0.01;
    if (!this.sys.pts.visible) return;
    const pos = this.sys.geo.attributes.position.array, al = this.sys.geo.attributes.aAlpha.array;
    for (let i = 0; i < this.n; i++) {
      const ph = this.ph[i];
      pos[i * 3] = this.base[i * 3] + Math.sin(t * 0.31 + ph) * 1.6 + Math.sin(t * 0.9 + ph * 2) * 0.4;
      pos[i * 3 + 1] = this.base[i * 3 + 1] + Math.sin(t * 0.47 + ph * 1.3) * 0.8;
      pos[i * 3 + 2] = this.base[i * 3 + 2] + Math.cos(t * 0.27 + ph) * 1.6;
      const blink = Math.max(0, Math.sin(t * (0.9 + (ph % 1)) + ph));
      al[i] = vis * Math.pow(blink, 3) * 1.4;
    }
    this.sys.geo.attributes.position.needsUpdate = true;
    this.sys.geo.attributes.aAlpha.needsUpdate = true;
  }
}

// ---------------------------------------------------------------- koi + lily pads

const KOI_PATTERNS = [
  { base: 0xf7f2ea, spots: 0xe0462b },
  { base: 0xff8a2a, spots: 0xffffff },
  { base: 0xf2b53a, spots: 0xf7d36a },
  { base: 0xf4f0ea, spots: 0x2a2a2e },
  { base: 0xe0462b, spots: 0xf7f2ea },
];

function koiGeometry(pat, seed) {
  const g = new VoxelGrid(8, 1, 5, 0, 0, 2, seed);
  const r = mulberry32(seed);
  const fin = 0xf5e9dc;
  const body = (x, z) => g.set(x, 0, z, r() < 0.45 ? pat.spots : pat.base, 0, 1, 0.04);
  g.set(0, 0, -1, fin, 0, 1, 0); g.set(0, 0, 1, fin, 0, 1, 0);
  g.set(1, 0, 0, pat.base, 0, 1, 0);
  body(2, 0);
  for (const x of [3, 4]) for (const z of [-1, 0, 1]) body(x, z);
  body(5, 0); g.set(5, 0, -1, pat.base); g.set(5, 0, 1, pat.base);
  g.set(6, 0, 0, pat.base, 0, 1, 0);
  g.set(4, 0, -2, fin, 0, 1, 0); g.set(4, 0, 2, fin, 0, 1, 0);
  const geo = meshGrid(g)[0];
  geo.translate(-3.5, -0.5, -0.5);
  return geo;
}

export class Koi {
  constructor(scene) {
    const mat = new THREE.MeshLambertMaterial({ vertexColors: true });
    this.fish = [];
    for (let i = 0; i < 6; i++) {
      const geo = koiGeometry(KOI_PATTERNS[i % KOI_PATTERNS.length], 300 + i);
      const m = new THREE.Mesh(geo, mat);
      m.scale.set(0.3, 0.22, 0.3);
      scene.add(m);
      this.fish.push({
        m, th: rand() * 6.3, w: (0.18 + rand() * 0.18) * (i % 2 ? 1 : -1),
        rx: 3.2 + rand() * 2.6, rz: 2.2 + rand() * 1.8, ph: rand() * 10,
        ox: (rand() - 0.5) * 2, oz: (rand() - 0.5) * 1.5, y: 0.05 + rand() * 0.45,
      });
    }
  }

  update(t) {
    for (const f of this.fish) {
      const th = f.th + f.w * t + 0.35 * Math.sin(t * 0.21 + f.ph);
      const x = POND.cx + f.ox + Math.cos(th) * f.rx, z = POND.cz + f.oz + Math.sin(th) * f.rz;
      const s = Math.sign(f.w);
      const vx = -Math.sin(th) * f.rx * s, vz = Math.cos(th) * f.rz * s;
      f.m.position.set(x, f.y, z);
      f.m.rotation.y = -Math.atan2(vz, vx) + Math.sin(t * 5 + f.ph) * 0.18;
    }
  }
}

export class LilyPads {
  constructor(scene, info) {
    const cells = info.water.filter(([x, z]) => {
      const dx = (x + 0.5 - POND.cx) / POND.rx, dz = (z + 0.5 - POND.cz) / POND.rz;
      const v = dx * dx + dz * dz;
      return v < 0.75 && v > 0.12;
    });
    const n = Math.min(16, cells.length);
    const pad = new THREE.InstancedMesh(new THREE.BoxGeometry(0.95, 0.1, 0.95), new THREE.MeshLambertMaterial({ color: 0xffffff }), n);
    const flower = new THREE.InstancedMesh(new THREE.BoxGeometry(0.42, 0.32, 0.42), new THREE.MeshLambertMaterial({ color: 0xffffff, emissive: 0x401828 }), 6);
    const c = new THREE.Color(), d = new THREE.Object3D();
    this.items = [];
    for (let i = 0; i < n; i++) {
      const [x, z] = cells.splice(Math.floor(rand() * cells.length), 1)[0];
      d.position.set(x + 0.5, WATER_Y + 0.03, z + 0.5);
      d.rotation.y = rand() * Math.PI;
      d.updateMatrix();
      pad.setMatrixAt(i, d.matrix);
      pad.setColorAt(i, c.setHex([0x3f8f3a, 0x4d9d42, 0x5aa84a][i % 3]));
      if (i < 6) {
        d.position.y += 0.2;
        d.updateMatrix();
        flower.setMatrixAt(i, d.matrix);
        flower.setColorAt(i, c.setHex(i % 2 ? 0xffb6d0 : 0xfff0f5));
      }
    }
    pad.receiveShadow = true;
    scene.add(pad, flower);
  }
}
