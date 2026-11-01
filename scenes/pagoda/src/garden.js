import { VoxelGrid, KIND_GLOW, mulberry32, noise2, noise3, fbm2, mixHex } from './voxels.js';

// World layout (1 unit = 1 voxel). Ground surface cells sit at y = 0, objects start at y = 1.
export const ISLAND_R = 36;
export const PAGODA_Z = -8;
export const POND = { cx: 17.5, cz: 12.5, rx: 8.6, rz: 6.8 };
export const WATER_Y = 0.82;
const STREAM = { z0: 11, z1: 13, x0: 22 };

export const SURF = { NONE: 0, GRASS: 1, PATH: 2, GRAVEL: 3, WATER: 4, SHORE: 5 };

const PAL = {
  dirt: [0x7d5537, 0x6e4a31, 0x8a613f, 0x664430],
  stone: [0x80838b, 0x72757d, 0x8d9096, 0x686b73, 0x5d6068],
  rock: [0x6d7078, 0x7b7e86, 0x8a8d94, 0x979aa0],
  moss: [0x5b8a3c, 0x4e7d36, 0x679843],
  path: [0xbfb8aa, 0xb3ac9e, 0xc9c2b4, 0xaaa395],
  pathEdge: [0x8f8a80, 0x858077],
  sand: [0xb5aa8a, 0xa99d7d, 0xbfb497],
  pondBed: [0x56705c, 0x4c6553, 0x63795e, 0x6f7b5c],
  // blob palettes are ordered dark -> light
  sakura: [0xe489ad, 0xee9fbe, 0xf5b3cb, 0xf9c7d9, 0xfbd8e5, 0xfde8f0],
  petalGround: [0xf3b9ce, 0xeea6c1, 0xf7cddc, 0xfadbe6],
  maple: [0xae2a24, 0xc73824, 0xd94e28, 0xe5672d, 0xef8638, 0xf5a647],
  green: [0x3b8135, 0x48943e, 0x56a245, 0x65ae4c, 0x77bb56],
  pine: [0x21503a, 0x295c40, 0x316845, 0x3b754c],
  bambooLeaf: [0x6ea544, 0x7fb550, 0x92c55d],
  bamboo: [0x7aa149, 0x84ab4f, 0x8db354],
  bark: [0x5a3f37, 0x4d362f, 0x634639],
  azalea: [0xc12f6c, 0xd43f82, 0xe35895, 0xef78ac],
  azaleaWhite: [0xcfc8d6, 0xe2dce8, 0xf2eef6, 0xfdfbff],
  flower: [0xffffff, 0xffe066, 0xff9fcd, 0xb99cff, 0xff7a7a],
  vine: [0x3c8536, 0x4a9640, 0x2f7230],
};

const C = {
  tile: 0x2c3039, ridge: 0x444a57, trim: 0xd2a347, gold: 0xe0b24a, bronze: 0xb48a3a,
  pillar: 0xc33b23, beam: 0xb3321e, sill: 0x6b3322, wall: 0xf0e6d2, wall2: 0xe6dac3,
  window: 0xffb357, bracket: 0xa22f1c, rafter: 0x8c2a1a, inner: 0x3a2a22, rail: 0xc8402a,
  platform: 0x9f998d, platformTop: 0xb3ad9f, toriiRed: 0xd23c22, black: 0x29282b,
  lanternStone: 0x8e8a82, lanternRoof: 0x77736b, lanternLight: 0xffc46e,
  wood: 0x8a5a3a, woodDark: 0x6b4430, gravel: 0xe2dccd, gravelLine: 0xd2cab8, curb: 0x9a958b,
};

const pick = (arr, r) => arr[Math.min(arr.length - 1, Math.floor(r * arr.length))];
const clamp = (v, a, b) => (v < a ? a : v > b ? b : v);

export function islandR(x, z, R = ISLAND_R, seed = 0) {
  const px = Math.abs(x + 0.5) / R, pz = Math.abs(z + 0.5) / R;
  const r = Math.pow(Math.pow(px, 3.4) + Math.pow(pz, 3.4), 1 / 3.4);
  return r + (fbm2(x * 0.09, z * 0.09, 2, seed + 11) - 0.5) * 0.07;
}

function hill(x, z, cx, cz, r, h) {
  return h * Math.exp(-((x - cx) ** 2 + (z - cz) ** 2) / (r * r));
}

function pondValue(x, z) {
  const dx = (x + 0.5 - POND.cx) / POND.rx, dz = (z + 0.5 - POND.cz) / POND.rz;
  return dx * dx + dz * dz + (noise2(x * 0.35, z * 0.35, 77) - 0.5) * 0.35;
}

// ---------------------------------------------------------------- context shared by all builders

function makeCtx(g, half, seed) {
  const H = new Int16Array(half * 2 * half * 2).fill(-999);
  const surf = new Uint8Array(half * 2 * half * 2);
  const ci = (x, z) => {
    const ix = Math.floor(x) + half, iz = Math.floor(z) + half;
    return ix < 0 || iz < 0 || ix >= half * 2 || iz >= half * 2 ? -1 : iz * half * 2 + ix;
  };
  return {
    g, H, surf, half, ci,
    rnd: mulberry32(seed),
    groundAt(x, z) {
      const i = ci(x, z);
      return i < 0 ? -999 : H[i];
    },
    spawns: [],
    lanterns: [],
    windows: [],
  };
}

// ---------------------------------------------------------------- primitives

function line(ctx, x0, y0, z0, x1, y1, z1, color, thick = false, sway = 0) {
  const len = Math.hypot(x1 - x0, y1 - y0, z1 - z0);
  const steps = Math.ceil(len * 2.5) + 1;
  for (let i = 0; i <= steps; i++) {
    const t = i / steps;
    const x = x0 + (x1 - x0) * t, y = y0 + (y1 - y0) * t, z = z0 + (z1 - z0) * t;
    if (thick) {
      for (const ox of [-0.5, 0.5]) for (const oz of [-0.5, 0.5]) ctx.g.set(x + ox, y, z + oz, color, sway, 1, 0.08);
    } else {
      ctx.g.set(x, y, z, color, sway, 1, 0.08);
    }
  }
}

function blob(ctx, cx, cy, cz, r, ry, palette, opt = {}) {
  const { g } = ctx;
  const seed = opt.seed || 0;
  const sway = opt.sway ?? 1;
  const cells = [];
  for (let y = Math.floor(cy - ry - 1); y <= Math.ceil(cy + ry + 1); y++) {
    for (let z = Math.floor(cz - r - 1); z <= Math.ceil(cz + r + 1); z++) {
      for (let x = Math.floor(cx - r - 1); x <= Math.ceil(cx + r + 1); x++) {
        const dx = (x + 0.5 - cx) / r, dy = (y + 0.5 - cy) / ry, dz = (z + 0.5 - cz) / r;
        const d = Math.sqrt(dx * dx + dy * dy + dz * dz);
        const n = noise3(x * 0.33, y * 0.33, z * 0.33, seed);
        if (d > 0.8 + (n - 0.5) * 0.6) continue;
        if (opt.floor !== undefined && y < opt.floor) continue;
        if (g.get(x, y, z)) continue;
        const t = clamp(0.45 + dy * 0.38 + (noise3(x * 0.2, y * 0.2, z * 0.2, seed + 5) - 0.5) * 0.9, 0, 0.999);
        g.set(x, y, z, palette[Math.floor(t * palette.length)], sway, 1, 0.05);
        cells.push([x, y, z]);
        if (opt.spawns && ctx.rnd() < 0.14) opt.spawns.push([x + 0.5, y + 0.2, z + 0.5]);
      }
    }
  }
  if (opt.droop && cells.length) {
    // weeping strands hanging from the bottom of the canopy
    const count = Math.round(r * r * 0.9);
    for (let k = 0; k < count; k++) {
      const [x, , z] = cells[Math.floor(ctx.rnd() * cells.length)];
      let low = 999;
      for (const c of cells) if (c[0] === x && c[2] === z && c[1] < low) low = c[1];
      const len = 1 + Math.floor(ctx.rnd() * 3);
      for (let j = 1; j <= len; j++) {
        if (g.get(x, low - j, z)) break;
        g.set(x, low - j, z, palette[Math.min(palette.length - 1, 1 + Math.floor(ctx.rnd() * 3))], sway, 1, 0.05);
      }
    }
  }
  return cells;
}

// ---------------------------------------------------------------- trees & plants

function tree(ctx, x, z, o) {
  const r = mulberry32(o.seed);
  const gy = ctx.groundAt(x, z) + 1;
  let px = x + 0.5, pz = z + 0.5;
  const lean = r() * Math.PI * 2;
  const trunk = [];
  for (let i = 0; i < o.height; i++) {
    const thick = i < o.height * (o.thick ?? 0.5);
    if (thick) {
      for (const ox of [-0.5, 0.5]) for (const oz of [-0.5, 0.5]) ctx.g.set(px + ox, gy + i, pz + oz, pick(PAL.bark, r()), 0, 1, 0.06);
    } else {
      ctx.g.set(px, gy + i, pz, pick(PAL.bark, r()), 0, 1, 0.06);
    }
    trunk.push([px, gy + i, pz]);
    px += Math.cos(lean) * 0.18 * r();
    pz += Math.sin(lean) * 0.18 * r();
  }
  const top = trunk[trunk.length - 1];
  const leafOpt = { seed: o.seed, spawns: o.spawns, droop: o.droop };
  const a0 = r() * Math.PI * 2;
  for (let b = 0; b < o.branches; b++) {
    const a = a0 + (b * Math.PI * 2) / o.branches + (r() - 0.5) * 0.7;
    const len = o.spread * (0.75 + r() * 0.45);
    const start = trunk[Math.floor(trunk.length * (0.55 + r() * 0.35))];
    const ex = start[0] + Math.cos(a) * len, ez = start[2] + Math.sin(a) * len;
    const ey = start[1] + len * (0.35 + r() * 0.35);
    line(ctx, start[0], start[1], start[2], ex, ey, ez, pick(PAL.bark, r()));
    blob(ctx, ex, ey + 0.6, ez, o.blob * (0.8 + r() * 0.35), o.blob * 0.7, o.palette, leafOpt);
  }
  blob(ctx, top[0], top[1] + 1.2, top[2], o.blob * 1.12, o.blob * 0.8, o.palette, leafOpt);
  return { x: x + 0.5, z: z + 0.5, r: o.spread + o.blob };
}

function sakura(ctx, x, z, size, seed) {
  const s = [
    { height: 5, branches: 3, spread: 3.2, blob: 2.7 },
    { height: 7, branches: 4, spread: 4.6, blob: 3.4 },
    { height: 9, branches: 5, spread: 6, blob: 4.1 },
  ][size];
  const t = tree(ctx, x, z, { ...s, palette: PAL.sakura, seed, spawns: ctx.spawns, droop: true });
  ctx.sakuraTrees.push(t);
}

function maple(ctx, x, z, size, seed) {
  const s = [{ height: 5, branches: 3, spread: 3, blob: 2.7 }, { height: 7, branches: 3, spread: 4.2, blob: 3.5 }][size];
  tree(ctx, x, z, { ...s, palette: PAL.maple, seed });
}

function greenTree(ctx, x, z, size, seed) {
  const s = [{ height: 6, branches: 3, spread: 2.6, blob: 3 }, { height: 8, branches: 3, spread: 3.6, blob: 3.9 }][size];
  tree(ctx, x, z, { ...s, palette: PAL.green, seed });
}

function pine(ctx, x, z, size, seed) {
  const r = mulberry32(seed);
  const gy = ctx.groundAt(x, z) + 1;
  const h = Math.round(5 + size * 3.5);
  let px = x + 0.5, pz = z + 0.5;
  const pts = [];
  for (let i = 0; i < h; i++) {
    if (i < 2 + size) {
      for (const ox of [-0.5, 0.5]) for (const oz of [-0.5, 0.5]) ctx.g.set(px + ox, gy + i, pz + oz, pick(PAL.bark, r()), 0, 1, 0.06);
    } else ctx.g.set(px, gy + i, pz, pick(PAL.bark, r()), 0, 1, 0.06);
    pts.push([px, gy + i, pz]);
    px += (r() - 0.5) * 0.9;
    pz += (r() - 0.5) * 0.9;
  }
  const pads = 3 + size * 2;
  for (let k = 0; k < pads; k++) {
    const t = 0.35 + (0.6 * k) / pads;
    const base = pts[Math.floor(t * (h - 1))];
    const a = r() * Math.PI * 2 + k * 2.4;
    const len = (2.2 + r() * 2.4) * (1.15 - t * 0.5) * (0.8 + size * 0.35);
    const ex = base[0] + Math.cos(a) * len, ez = base[2] + Math.sin(a) * len, ey = base[1] + 0.6 + r();
    line(ctx, base[0], base[1], base[2], ex, ey, ez, pick(PAL.bark, r()));
    blob(ctx, ex, ey + 0.9, ez, 2.3 + size * 0.8 + r() * 0.6, 1.25, PAL.pine, { seed: seed + k, sway: 0.55 });
  }
  blob(ctx, px, gy + h + 0.6, pz, 2.8 + size * 0.7, 1.5, PAL.pine, { seed: seed + 99, sway: 0.55 });
}

function bamboo(ctx, x0, z0, x1, z1, count, seed) {
  const r = mulberry32(seed);
  for (let n = 0; n < count; n++) {
    const x = Math.floor(x0 + r() * (x1 - x0)), z = Math.floor(z0 + r() * (z1 - z0));
    const G = ctx.groundAt(x, z);
    if (G < -100 || ctx.surf[ctx.ci(x, z)] !== SURF.GRASS || ctx.g.get(x, G + 1, z)) continue;
    const h = 13 + Math.floor(r() * 8);
    for (let i = 0; i < h; i++) {
      const c = i % 4 === 3 ? 0xa8c96e : i > h - 3 ? 0x9cc263 : pick(PAL.bamboo, r());
      ctx.g.set(x, G + 1 + i, z, c, Math.pow(i / h, 1.6) * 0.9, 1, 0.04);
      if (i > h * 0.5 && r() < 0.55) {
        const dx = Math.round((r() - 0.5) * 3), dz = Math.round((r() - 0.5) * 3);
        for (let k = 0; k < 2; k++) {
          ctx.g.setIfEmpty(x + dx + (k ? Math.sign(dx) : 0), G + 1 + i - k, z + dz, pick(PAL.bambooLeaf, r()), 0.6 + (i / h) * 0.4, 1, 0.05);
        }
      }
    }
  }
}

function bush(ctx, x, z, r, palette, seed) {
  const gy = ctx.groundAt(x, z) + 1;
  blob(ctx, x + 0.5, gy + r * 0.45, z + 0.5, r, r * 0.75, palette, { seed, sway: 0.18, floor: gy });
  // a few green leaves mixed in
  const rr = mulberry32(seed);
  for (let k = 0; k < r * 4; k++) {
    const a = rr() * Math.PI * 2;
    ctx.g.recolor(Math.floor(x + 0.5 + Math.cos(a) * r * 0.85), Math.floor(gy + rr() * r * 0.6), Math.floor(z + 0.5 + Math.sin(a) * r * 0.85), pick(PAL.green, rr()));
  }
}

function rock(ctx, x, z, r, seed) {
  const gy = ctx.groundAt(x, z) + 1;
  const cells = blob(ctx, x + 0.5, gy + r * 0.2 - 0.5, z + 0.5, r, r * 0.75, PAL.rock, { seed, sway: 0, floor: gy });
  const rr = mulberry32(seed + 3);
  for (const [cx, cy, cz] of cells) {
    if (!ctx.g.get(cx, cy + 1, cz) && rr() < 0.45) ctx.g.recolor(cx, cy, cz, pick(PAL.moss, rr()));
  }
}

// ---------------------------------------------------------------- architecture

function lantern(ctx, x, z) {
  const g = ctx.g;
  const y = ctx.groundAt(x, z) + 1;
  const sq = (yy, hw, c, kind = 1, round = false) => {
    for (let dx = -hw; dx <= hw; dx++) for (let dz = -hw; dz <= hw; dz++) {
      if (round && Math.abs(dx) === hw && Math.abs(dz) === hw) continue;
      g.set(x + dx, yy, z + dz, c, 0, kind, 0.05);
    }
  };
  sq(y, 1, C.lanternStone);
  g.set(x, y + 1, z, C.lanternStone);
  g.set(x, y + 2, z, C.lanternStone);
  sq(y + 3, 1, C.lanternStone);
  for (let dx = -1; dx <= 1; dx++) for (let dz = -1; dz <= 1; dz++) {
    const corner = Math.abs(dx) === 1 && Math.abs(dz) === 1;
    g.set(x + dx, y + 4, z + dz, corner ? C.lanternStone : C.lanternLight, 0, corner ? 1 : KIND_GLOW, 0.02);
  }
  sq(y + 5, 2, C.lanternRoof, 1, true);
  sq(y + 6, 1, C.lanternRoof);
  g.set(x, y + 7, z, C.lanternRoof);
  ctx.lanterns.push({ x: x + 0.5, y: y + 4.5, z: z + 0.5 });
}

function pagoda(ctx) {
  const g = ctx.g;
  const cz = PAGODA_Z;
  const box = (x0, x1, y0, y1, z0, z1, c, jit = 0.04) => {
    for (let y = y0; y <= y1; y++) for (let z = z0; z <= z1; z++) for (let x = x0; x <= x1; x++) g.set(x, y, z, c, 0, 1, jit);
  };
  // stone base
  for (let y = 1; y <= 2; y++) {
    for (let x = -10; x <= 10; x++) for (let z = cz - 10; z <= cz + 10; z++) {
      const top = y === 2;
      const slab = top && ((x + 20) % 4 === 0 || (z + 20) % 4 === 0);
      g.set(x, y, z, top ? (slab ? C.platform : C.platformTop) : C.platform, 0, 1, 0.05);
    }
  }
  box(-8, 8, 3, 3, cz - 8, cz + 8, C.platformTop);
  box(-3, 3, 1, 1, cz + 11, cz + 12, C.platform);
  box(-3, 3, 2, 2, cz + 11, cz + 11, C.platform);
  box(-2, 2, 3, 3, cz + 9, cz + 9, C.platformTop);

  const B = [7, 6, 5, 4, 3];
  let y0 = 4;
  const along = (x, dz) => (Math.abs(x) >= Math.abs(dz) ? dz : x);
  for (let i = 0; i < 5; i++) {
    const b = B[i], h = i === 0 ? 5 : 4, R = b + 4;
    // walls, pillars, windows
    for (let yy = y0; yy < y0 + h; yy++) {
      const row = yy - y0;
      for (let x = -b; x <= b; x++) for (let z = cz - b; z <= cz + b; z++) {
        const dz = z - cz;
        const ex = Math.abs(x) === b, ez = Math.abs(dz) === b;
        if (!ex && !ez) { g.set(x, yy, z, C.inner, 0, 1, 0); continue; }
        const t = Math.abs(ex ? dz : x);
        let c, kind = 1;
        if (row === h - 1) c = C.beam;
        else if (row === 0) c = C.sill;
        else if ((ex && ez) || t === 2 || (b >= 6 && t === 5)) c = C.pillar;
        else if (t <= 1) { c = C.window; kind = KIND_GLOW; }
        else c = row % 2 === 0 ? C.wall2 : C.wall;
        g.set(x, yy, z, c, 0, kind, kind === KIND_GLOW ? 0.02 : 0.03);
      }
    }
    const wy = y0 + h / 2;
    for (const [wx, wz] of [[0.5, cz + b + 1.3], [0.5, cz - b - 0.3], [b + 1.3, cz + 0.5], [-b - 0.3, cz + 0.5]]) {
      ctx.windows.push({ x: wx, y: wy, z: wz, s: i === 0 ? 4.2 : 3.2 });
    }

    // bracket + rafter layer under the eaves
    const yb = y0 + h;
    for (let x = -(R - 1); x <= R - 1; x++) for (let z = cz - (R - 1); z <= cz + (R - 1); z++) {
      const dz = z - cz;
      const m = Math.max(Math.abs(x), Math.abs(dz));
      const t = along(x, dz);
      if (m <= b) g.set(x, yb, z, C.inner, 0, 1, 0);
      else if (m === b + 1) g.set(x, yb, z, Math.abs(t) % 2 === 0 ? C.gold : C.bracket, 0, 1, 0.04);
      else if (Math.abs(x) === Math.abs(dz) || Math.abs(t) % 2 === 0) g.set(x, yb, z, C.rafter, 0, 1, 0.05);
    }

    // roof
    const ry = yb + 1;
    const layer = (yy, hw, edgeTrim) => {
      for (let x = -hw; x <= hw; x++) for (let z = cz - hw; z <= cz + hw; z++) {
        const dz = z - cz;
        const m = Math.max(Math.abs(x), Math.abs(dz));
        let c = C.tile;
        if (edgeTrim && m === hw) c = C.trim;
        else if (Math.abs(x) === Math.abs(dz)) c = C.ridge;
        g.set(x, yy, z, c, 0, 1, c === C.tile ? 0.07 : 0.04);
      }
    };
    layer(ry, R, true);
    layer(ry + 1, R - 2, false);
    for (const sx of [-1, 1]) for (const sz of [-1, 1]) {
      g.set(sx * R, ry + 1, cz + sz * R, C.trim, 0, 1, 0.03);
      g.set(sx * (R - 1), ry + 1, cz + sz * R, C.tile, 0, 1, 0.05);
      g.set(sx * R, ry + 1, cz + sz * (R - 1), C.tile, 0, 1, 0.05);
      g.set(sx * (R - 1), ry + 1, cz + sz * (R - 1), C.ridge, 0, 1, 0.03);
      g.set(sx * (R + 1), ry + 2, cz + sz * (R + 1), C.trim, 0, 1, 0.03);
      g.set(sx * R, yb, cz + sz * R, C.gold, 0, 1, 0.02); // wind bell
    }
    if (i < 4) {
      layer(ry + 2, R - 4, false);
      // balcony railing of the next storey
      const rh = R - 4;
      for (let x = -rh; x <= rh; x++) for (let z = cz - rh; z <= cz + rh; z++) {
        const dz = z - cz;
        if (Math.max(Math.abs(x), Math.abs(dz)) !== rh) continue;
        const t = along(x, dz);
        if (Math.abs(t) % 2 === 0 || Math.abs(x) === Math.abs(dz)) g.set(x, ry + 3, z, C.rail, 0, 1, 0.04);
      }
      y0 = ry + 2;
    } else {
      layer(ry + 2, 3, false);
      layer(ry + 3, 2, false);
      layer(ry + 4, 1, false);
      box(-1, 1, ry + 5, ry + 5, cz - 1, cz + 1, C.bronze);
      g.set(0, ry + 6, cz, C.bronze);
      for (let k = 0; k < 11; k++) {
        const yy = ry + 7 + k;
        g.set(0, yy, cz, C.bronze, 0, 1, 0.03);
        if (k % 2 === 1 && k < 10) for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) g.set(dx, yy, cz + dz, C.gold, 0, 1, 0.03);
      }
      for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) g.set(dx, ry + 18, cz + dz, C.gold, 0, 1, 0.02);
      g.set(0, ry + 18, cz, C.gold);
      g.set(0, ry + 19, cz, C.gold);
      g.set(0, ry + 20, cz, 0xf2cf6a, 0, 1, 0);
      ctx.pagodaTop = ry + 20;
    }
  }
  ctx.doorLight = { x: 0.5, y: 6, z: cz + 9.5 };
}

function torii(ctx, z0) {
  const g = ctx.g;
  for (const xs of [[-6, -5], [5, 6]]) for (const x of xs) for (const z of [z0, z0 + 1]) {
    g.set(x, 1, z, C.black, 0, 1, 0.03);
    for (let y = 2; y <= 12; y++) g.set(x, y, z, C.toriiRed, 0, 1, 0.03);
  }
  for (let x = -8; x <= 8; x++) for (const z of [z0, z0 + 1]) g.set(x, 9, z, C.toriiRed, 0, 1, 0.03);
  for (let x = -9; x <= 9; x++) for (const z of [z0, z0 + 1]) g.set(x, 12, z, C.toriiRed, 0, 1, 0.03);
  for (let x = -10; x <= 10; x++) for (const z of [z0, z0 + 1]) g.set(x, 13, z, C.black, 0, 1, 0.03);
  for (const x of [-11, -10, 10, 11]) for (const z of [z0, z0 + 1]) g.set(x, 14, z, C.black, 0, 1, 0.03);
  for (let x = -1; x <= 1; x++) for (let y = 10; y <= 11; y++) for (const z of [z0, z0 + 1]) {
    g.set(x, y, z, x === 0 ? C.gold : C.black, 0, 1, 0.02);
  }
}

function bridge(ctx) {
  const g = ctx.g;
  for (let z = 6; z <= 18; z++) {
    const dz = (z - 12) / 6;
    const yDeck = 1 + Math.round(2.6 * (1 - dz * dz));
    for (let x = 27; x <= 31; x++) {
      const rail = x === 27 || x === 31;
      g.set(x, yDeck, z, rail ? C.rail : C.wood, 0, 1, 0.05);
      if (!isStream(x, z) && !pondValueLess(x, z)) for (let y = 1; y < yDeck; y++) g.set(x, y, z, C.woodDark, 0, 1, 0.05);
      if (rail) {
        g.set(x, yDeck + 1, z, C.rail, 0, 1, 0.04);
        if (z % 3 === 0 || z === 6 || z === 18) g.set(x, yDeck + 2, z, z === 6 || z === 18 ? C.gold : C.rail, 0, 1, 0.03);
      }
    }
  }
}

let streamEdgeX = 34;
const isStream = (x, z) => z >= STREAM.z0 && z <= STREAM.z1 && x >= STREAM.x0 && x <= streamEdgeX;
const pondValueLess = (x, z) => pondValue(x, z) < 1;
const isWater = (x, z) => pondValueLess(x, z) || isStream(x, z);

// ---------------------------------------------------------------- the main island

export function buildGarden() {
  const half = 48;
  const g = new VoxelGrid(96, 108, 96, 48, 40, 48, 7);
  const ctx = makeCtx(g, half, 4242);
  ctx.sakuraTrees = [];
  const { H, surf, ci } = ctx;
  const rnd = ctx.rnd;

  // where the stream leaves the island
  let xe = 20;
  while (islandR(xe + 1, 12) < 1) xe++;
  streamEdgeX = xe;
  const inIsland = (x, z) => (z >= 10 && z <= 14 && x > 20 ? x <= xe : islandR(x, z) < 1);

  const flatMask = (x, z) => {
    const bx = Math.max(-16 - x, 0, x - 16), bz = Math.max(PAGODA_Z - 16 - z, 0, z - (PAGODA_Z + 16));
    const dPag = Math.hypot(bx, bz);
    const dPath = z > 0 ? Math.max(0, Math.abs(x + 0.5) - 4) : 99;
    const pv = Math.sqrt(((x + 0.5 - POND.cx) / (POND.rx + 3)) ** 2 + ((z + 0.5 - POND.cz) / (POND.rz + 3)) ** 2);
    const dPond = Math.max(0, pv - 1) * 9;
    const dStream = x > 18 ? Math.max(0, Math.abs(z - 12) - 3) : 99;
    return clamp(Math.min(dPag, dPath, dPond, dStream) / 5, 0, 1);
  };
  const terrain = (x, z) => {
    let h = hill(x, z, -26, -20, 9, 3.4) + hill(x, z, 25, -23, 8, 2.6) + hill(x, z, -27, 23, 7, 2.2) +
      hill(x, z, 27, 27, 6, 1.8) + hill(x, z, 4, -31, 8, 2.0) + hill(x, z, -31, 2, 6, 1.6);
    h += (fbm2(x * 0.11, z * 0.11, 2, 5) - 0.5) * 1.4;
    return Math.max(0, Math.round(h * flatMask(x, z)));
  };

  // 1. island body
  for (let z = -half; z < half; z++) {
    for (let x = -half; x < half; x++) {
      if (!inIsland(x, z)) continue;
      const inner = clamp(1 - islandR(x, z), 0, 1);
      const G = terrain(x, z);
      const i = ci(x, z);
      H[i] = G;
      surf[i] = SURF.GRASS;
      let depth = 3 + 19 * Math.pow(inner, 1.15) + (fbm2(x * 0.13, z * 0.13, 3, 9) - 0.5) * 7 * inner;
      const sp = noise2(x * 0.45, z * 0.45, 33);
      if (sp > 0.7) depth += (sp - 0.7) * 26 * inner;
      const bottom = -Math.round(depth);
      const dirtDepth = 2 + (noise2(x * 0.3, z * 0.3, 4) > 0.5 ? 1 : 0);
      for (let y = bottom; y <= G; y++) {
        const dt = G - y;
        let c;
        if (dt === 0) {
          const n = fbm2(x * 0.07, z * 0.07, 3, 3);
          c = mixHex(0x4f9b3b, 0x80c858, clamp((n - 0.3) * 2.2, 0, 1));
          if (rnd() < 0.08) c = mixHex(c, 0x3f8a35, 0.5);
        } else if (dt <= dirtDepth) c = pick(PAL.dirt, rnd());
        else {
          const band = Math.floor((y + noise2(x * 0.2, z * 0.2, 8) * 3) / 3);
          c = PAL.stone[((band % PAL.stone.length) + PAL.stone.length) % PAL.stone.length];
          if (rnd() < 0.06) c = PAL.stone[Math.floor(rnd() * PAL.stone.length)];
        }
        g.set(x, y, z, c, 0, 1, dt === 0 ? 0.05 : 0.06);
      }
      // dangling roots under the island
      if (rnd() < 0.006 && inner > 0.3) {
        const len = 1 + Math.floor(rnd() * 3);
        for (let k = 1; k <= len; k++) g.set(x, bottom - k, z, k < len ? 0x6a4a33 : 0x7a5a3e, 0, 1, 0.08);
      }
    }
  }

  // 2. pond + stream
  const water = [];
  for (let z = -half; z < half; z++) for (let x = -half; x < half; x++) {
    const i = ci(x, z);
    if (H[i] < -100 || !isWater(x, z)) continue;
    for (let y = H[i]; y >= -1; y--) g.clear(x, y, z);
    g.set(x, -2, z, pick(PAL.pondBed, rnd()), 0, 1, 0.06);
    H[i] = -2;
    surf[i] = SURF.WATER;
    water.push([x, z]);
  }
  // shore
  for (const [x, z] of water) {
    for (let dz = -1; dz <= 1; dz++) for (let dx = -1; dx <= 1; dx++) {
      const i = ci(x + dx, z + dz);
      if (i < 0 || surf[i] !== SURF.GRASS) continue;
      surf[i] = SURF.SHORE;
      const G = H[i];
      g.recolor(x + dx, G, z + dz, pick(PAL.sand, rnd()));
      if (rnd() < 0.3 && !isStream(x + dx, z + dz - 1) && !isStream(x + dx, z + dz + 1)) {
        g.set(x + dx, G + 1, z + dz, pick(PAL.rock, rnd()), 0, 1, 0.06);
        if (rnd() < 0.25) g.set(x + dx, G + 2, z + dz, pick(PAL.rock, rnd()), 0, 1, 0.06);
      }
    }
  }

  // 3. gravel court around the pagoda
  for (let z = PAGODA_Z - 14; z <= PAGODA_Z + 14; z++) for (let x = -14; x <= 14; x++) {
    const i = ci(x, z);
    if (H[i] < -100) continue;
    surf[i] = SURF.GRAVEL;
    const k = Math.max(Math.abs(x), Math.abs(z - PAGODA_Z));
    const c = k === 14 ? C.curb : k % 2 === 0 ? C.gravelLine : C.gravel;
    g.recolor(x, H[i], z, c, 0.03);
  }

  // 4. main path from the gravel court to the front edge
  for (let z = PAGODA_Z + 15; z < half; z++) for (let x = -2; x <= 2; x++) {
    const i = ci(x, z);
    if (H[i] < -100) continue;
    surf[i] = SURF.PATH;
    const edge = Math.abs(x) === 2;
    const slab = Math.floor((z + (x > 0 ? 1 : 0)) / 2);
    const c = edge ? pick(PAL.pathEdge, rnd()) : PAL.path[(slab * 7 + 3) % PAL.path.length];
    g.recolor(x, H[i], z, (z + (x > 0 ? 1 : 0)) % 2 === 0 && !edge ? mixHex(c, 0x8f897c, 0.35) : c, 0.03);
  }
  // stepping stones towards the pond
  for (const [sx, sz] of [[3, 15], [5, 14], [7, 15]]) {
    for (let dz = 0; dz <= 1; dz++) for (let dx = 0; dx <= 1; dx++) {
      const i = ci(sx + dx, sz + dz);
      if (surf[i] === SURF.GRASS) { g.recolor(sx + dx, H[i], sz + dz, 0xaaa69c); surf[i] = SURF.PATH; }
    }
  }

  // 5. buildings
  pagoda(ctx);
  torii(ctx, 25);
  bridge(ctx);
  for (const [lx, lz] of [[-6, 9], [6, 9], [9, 21], [-14, 27]]) lantern(ctx, lx, lz);

  // 6. vegetation
  sakura(ctx, -19, 15, 2, 11);
  sakura(ctx, -23, -14, 2, 12);
  sakura(ctx, 22, -19, 2, 13);
  sakura(ctx, 27, 26, 1, 14);
  sakura(ctx, -26, 26, 0, 15);
  sakura(ctx, 12, 31, 0, 16);
  maple(ctx, -31, 0, 1, 21);
  maple(ctx, 31, -3, 1, 22);
  maple(ctx, -11, -30, 0, 23);
  greenTree(ctx, 13, -31, 1, 31);
  greenTree(ctx, 26, -26, 0, 32);
  greenTree(ctx, -33, 13, 0, 33);
  pine(ctx, 1, -31, 1, 41);
  pine(ctx, -8, 21, 0, 42);
  pine(ctx, 31, 20, 0, 43);
  bamboo(ctx, -31, -32, -21, -25, 26, 51);

  const bushes = [
    [-5, 13, 1.9, PAL.azalea], [-5, 18, 1.6, PAL.azaleaWhite], [5, 19, 1.7, PAL.azalea], [-4, 23, 1.5, PAL.azalea],
    [-17, 7, 2.0, PAL.azalea], [-17, 0, 1.7, PAL.azaleaWhite], [-17, -13, 1.9, PAL.azalea], [17, -11, 1.8, PAL.azalea],
    [17, 1, 1.6, PAL.azaleaWhite], [-10, 31, 1.6, PAL.azalea], [21, 23, 1.5, PAL.azaleaWhite], [-29, 10, 1.6, PAL.azalea],
  ];
  bushes.forEach(([x, z, r, p], k) => bush(ctx, x, z, r, p, 61 + k));

  const rocks = [[-11, 3, 1.6], [11, -20, 1.9], [-12, -19, 1.4], [33, 8, 2.0], [-33, -8, 2.0], [19, 22, 1.4], [-20, -30, 1.7], [30, 12 - 6, 1.2]];
  rocks.forEach(([x, z, r], k) => rock(ctx, x, z, r, 71 + k));

  // 7. ground details: fallen petals, flowers, grass tufts, hanging vines
  for (const t of ctx.sakuraTrees) {
    const R = t.r + 2;
    for (let z = Math.floor(t.z - R); z <= Math.ceil(t.z + R); z++) for (let x = Math.floor(t.x - R); x <= Math.ceil(t.x + R); x++) {
      const i = ci(x, z);
      if (i < 0 || surf[i] !== SURF.GRASS) continue;
      const d = Math.hypot(x + 0.5 - t.x, z + 0.5 - t.z) / R;
      if (d < 1 && rnd() < 0.42 * (1 - d)) g.recolor(x, H[i], z, pick(PAL.petalGround, rnd()), 0.03);
    }
  }
  for (let z = -half; z < half; z++) for (let x = -half; x < half; x++) {
    const i = ci(x, z);
    if (surf[i] !== SURF.GRASS) continue;
    const G = H[i];
    if (g.get(x, G + 1, z)) continue;
    const r = rnd();
    if (r < 0.03) g.set(x, G + 1, z, mixHex(0x4a9a3e, 0x6fbb4e, rnd()), 0.35, 1, 0.06);
    else if (r < 0.047) g.set(x, G + 1, z, pick(PAL.flower, rnd()), 0.3, 1, 0.04);
    // vines down the island wall
    for (const [dx, dz] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
      if (ci(x + dx, z + dz) >= 0 && H[ci(x + dx, z + dz)] < -100 && rnd() < 0.09) {
        const len = 2 + Math.floor(rnd() * 6);
        for (let k = 0; k < len; k++) g.setIfEmpty(x + dx, G - k, z + dz, pick(PAL.vine, rnd()), 0.12 + k * 0.08, 1, 0.06);
      }
    }
  }

  return {
    grid: g,
    info: {
      H, surf, ci, half,
      water,
      spawns: ctx.spawns,
      lanterns: ctx.lanterns,
      windows: ctx.windows,
      doorLight: ctx.doorLight,
      pagodaTop: ctx.pagodaTop,
      waterfall: { x: xe + 1, z0: STREAM.z0, z1: STREAM.z1 + 1, y: WATER_Y },
      sakuraTrees: ctx.sakuraTrees,
    },
  };
}

// ---------------------------------------------------------------- small floating islands

export function buildMiniIsland(type, seed) {
  const half = 11;
  const g = new VoxelGrid(22, 30, 22, 11, 16, 11, seed);
  const ctx = makeCtx(g, half, seed);
  ctx.sakuraTrees = [];
  const R = type === 'shrine' ? 7 : 5.2;
  const rnd = ctx.rnd;
  for (let z = -half; z < half; z++) for (let x = -half; x < half; x++) {
    const rr = islandR(x, z, R, seed);
    if (rr >= 1) continue;
    const inner = 1 - rr;
    const i = ctx.ci(x, z);
    ctx.H[i] = 0;
    ctx.surf[i] = SURF.GRASS;
    const bottom = -Math.round(2 + R * 1.7 * Math.pow(inner, 1.1) + (noise2(x * 0.5, z * 0.5, seed) - 0.5) * 3 * inner);
    for (let y = bottom; y <= 0; y++) {
      const dt = -y;
      const c = dt === 0 ? mixHex(0x56a240, 0x7cc455, rnd()) : dt <= 2 ? pick(PAL.dirt, rnd()) : pick(PAL.stone, rnd());
      g.set(x, y, z, c, 0, 1, 0.05);
    }
  }
  if (type === 'sakura') {
    sakura(ctx, 0, 0, 0, seed + 1);
    rock(ctx, 2, -2, 1.1, seed + 2);
  } else if (type === 'lantern') {
    lantern(ctx, 0, 0);
    bush(ctx, 2, 2, 1.3, PAL.azalea, seed + 3);
    rock(ctx, -2, 2, 1.0, seed + 4);
  } else if (type === 'shrine') {
    pine(ctx, -2, -1, 0, seed + 5);
    rock(ctx, 3, 2, 1.4, seed + 6);
    bush(ctx, 2, -3, 1.2, PAL.azaleaWhite, seed + 7);
  }
  for (let z = -half; z < half; z++) for (let x = -half; x < half; x++) {
    const i = ctx.ci(x, z);
    if (ctx.surf[i] === SURF.GRASS && !g.get(x, 1, z) && rnd() < 0.06) g.set(x, 1, z, rnd() < 0.6 ? 0x4f9b3e : pick(PAL.flower, rnd()), 0.3, 1, 0.05);
  }
  return { grid: g, lanterns: ctx.lanterns, spawns: ctx.spawns };
}

// ---------------------------------------------------------------- voxel clouds

export function buildCloud(seed, len) {
  const r = mulberry32(seed);
  const w = len, d = Math.max(6, Math.round(len * (0.45 + r() * 0.2))), h = 7;
  const g = new VoxelGrid(w + 6, h + 4, d + 6, 3, 0, 3, seed);
  const blobs = 4 + Math.floor(r() * 4);
  for (let k = 0; k < blobs; k++) {
    const cx = w * (0.12 + 0.76 * r()), cz = d * (0.25 + 0.5 * r());
    const rr = 1.8 + r() * 2.6, cy = 0.6 + rr * 0.45;
    for (let y = 0; y <= h + 2; y++) for (let z = -3; z < d + 3; z++) for (let x = -3; x < w + 3; x++) {
      const dx = (x + 0.5 - cx) / (rr * 1.45), dy = (y + 0.5 - cy) / (rr * 0.8), dz = (z + 0.5 - cz) / rr;
      if (dx * dx + dy * dy + dz * dz > 1) continue;
      g.set(x, y, z, y < 1 ? 0xdfe5ef : 0xffffff, 0, 1, 0.02);
    }
  }
  return { grid: g, w, d };
}
