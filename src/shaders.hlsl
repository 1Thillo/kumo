// Kumo shaders (Shader Model 5).
//
// Lighting, tone mapping and fog deliberately follow three.js (MeshLambertMaterial,
// NeutralToneMapping, linear fog mixed in after the sRGB transfer) so a baked scene looks the
// same as in its browser preview. Colours arrive in linear space; every pixel shader writes
// sRGB-encoded values into a plain UNORM target, exactly like a WebGL canvas.

#define PI 3.14159265

cbuffer Frame : register(b0) {
  float4x4 shadowVP;
  float4 keyDir;       // xyz: direction towards the key light, w: time (s)
  float4 keyCol;       // rgb: key colour * intensity, w: wind strength
  float4 hemiSky;      // rgb: hemisphere sky colour * intensity, w: exposure
  float4 hemiGround;   // rgb, w: glow amount for emissive voxels
  float4 fogCol;       // rgb, w: diffuse factor for glowing voxels
  float4 fogRange;     // x near, y far, z 1/shadow map size, w normal bias
  float4 shadowCfg;    // x depth bias, y shadows enabled, z point lights active
  float4 plPos[4];     // xyz position, w cutoff distance
  float4 plCol[4];     // rgb colour * intensity, w decay
  float4 skyZen;       // rgb, w star visibility
  float4 skyHor;       // rgb, w sun visibility
  float4 skyGnd;       // rgb, w moon visibility
  float4 sunCol;
  float4 sunDir;
  float4 moonDir;
  float4 waterSky;     // rgb, w specular strength
  float4 waterDeep;    // rgb, w light level
  float4 waterShallow; // rgb, w glow
  float4 waterSunDir;  // xyz, w waterfall light
  float4 waterSunCol;
  float4 waterGlow;
  float4 fallCol;
  float4 petalEmissive;
  float4 petalCol[5];
};

cbuffer View : register(b1) {
  float4x4 viewProj;
  float4x4 invViewProj;
  float4 camPos;
  float4 camRight;
  float4 camUp;
  float4 viewInfo;     // xy: 1 / source size, z: FXAA on
};

cbuffer Object : register(b2) {
  float4x4 world;
  float4 emissive;     // rgb, w: receives shadows
  float4 objFlags;     // x: fog
};

Texture2D<float> shadowMap : register(t0);
Texture2D postSrc : register(t2);
SamplerComparisonState shadowSmp : register(s0);
SamplerState linearSmp : register(s1);

static const float2 QUAD[6] = { float2(-1, -1), float2(1, -1), float2(1, 1), float2(-1, -1), float2(1, 1), float2(-1, 1) };

// ---------------------------------------------------------------- helpers

float ss(float a, float b, float x) { float t = saturate((x - a) / (b - a)); return t * t * (3.0 - 2.0 * t); }

float3 toLinear(float3 c) { return c <= 0.04045 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4); }
float3 toSrgb(float3 c) { c = max(c, 0.0); return c <= 0.0031308 ? c * 12.92 : pow(c, 0.41666) * 1.055 - 0.055; }

float3 neutral(float3 color) {
  const float start = 0.8 - 0.04;
  const float desat = 0.15;
  color *= hemiSky.w;
  float x = min(color.r, min(color.g, color.b));
  float offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
  color -= offset;
  float peak = max(color.r, max(color.g, color.b));
  if (peak < start) return color;
  float d = 1.0 - start;
  float newPeak = 1.0 - d * d / (peak + d - start);
  color *= newPeak / peak;
  float g = 1.0 - 1.0 / (desat * (peak - newPeak) + 1.0);
  return lerp(color, newPeak.xxx, g);
}

float3 finish(float3 c, float depth, float fog) {
  float3 o = toSrgb(neutral(c));
  if (fog > 0.5) o = lerp(o, fogCol.rgb, ss(fogRange.x, fogRange.y, depth));
  return o;
}

float shadowAt(float3 wpos, float3 n) {
  float4 c = mul(shadowVP, float4(wpos + n * fogRange.w, 1.0));
  float3 p = c.xyz / c.w;
  float2 uv = float2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
  float z = p.z + shadowCfg.x;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || z > 1.0) return 1.0;
  float t = fogRange.z;
  float s = shadowMap.SampleCmpLevelZero(shadowSmp, uv, z) * 2.0;
  s += shadowMap.SampleCmpLevelZero(shadowSmp, uv + float2(-t, -t), z);
  s += shadowMap.SampleCmpLevelZero(shadowSmp, uv + float2(t, -t), z);
  s += shadowMap.SampleCmpLevelZero(shadowSmp, uv + float2(-t, t), z);
  s += shadowMap.SampleCmpLevelZero(shadowSmp, uv + float2(t, t), z);
  return s / 6.0;
}

// Lambert diffuse (hemisphere + key light with shadow + point lights), three.js units.
float3 lambert(float3 albedo, float3 n, float3 wpos, bool receive) {
  float3 irr = lerp(hemiGround.rgb, hemiSky.rgb, 0.5 * n.y + 0.5);
  float ndl = saturate(dot(n, keyDir.xyz));
  if (ndl > 0.0) irr += ndl * keyCol.rgb * (receive && shadowCfg.y > 0.5 ? shadowAt(wpos, n) : 1.0);
  if (shadowCfg.z > 0.5) {
    [unroll] for (int k = 0; k < 4; k++) {
      float3 L = plPos[k].xyz - wpos;
      float d = length(L);
      float att = 1.0 / max(pow(d, plCol[k].w), 0.01);
      float cut = saturate(1.0 - pow(d / plPos[k].w, 4.0));
      irr += saturate(dot(n, L / d)) * plCol[k].rgb * att * cut * cut;
    }
  }
  return irr * albedo / PI;
}

// ---------------------------------------------------------------- voxel meshes

struct VoxelIn { float3 pos : POSITION; float4 nrm : NORMAL; float4 col : COLOR; };
struct VoxelOut {
  float4 pos : SV_Position;
  float3 wpos : TEXCOORD0;
  float3 nrm : TEXCOORD1;
  float3 col : TEXCOORD2;
  float2 misc : TEXCOORD3; // x: glow, y: view depth
};

VoxelOut VSVoxel(VoxelIn i) {
  float3 p = i.pos;
  float sway = i.col.a;
  if (sway > 0.0) {
    float t = keyDir.w;
    float3 q = p;
    float gust = 0.65 + 0.35 * sin(t * 0.31 + q.x * 0.015 + q.z * 0.01);
    float ph = t * 1.15 + q.x * 0.13 + q.z * 0.11;
    float s = sway * keyCol.w * gust;
    p.x += (sin(ph) * 0.15 + sin(t * 2.8 + q.y * 0.5 + q.z * 0.3) * 0.05) * s;
    p.z += (cos(ph * 0.9 + 1.3) * 0.11 + cos(t * 3.2 + q.x * 0.45) * 0.04) * s;
    p.y += sin(t * 2.1 + q.x * 0.6 + q.z * 0.4) * 0.03 * s;
  }
  VoxelOut o;
  float4 w = mul(world, float4(p, 1.0));
  o.wpos = w.xyz;
  o.nrm = normalize(mul((float3x3)world, i.nrm.xyz));
  o.col = toLinear(i.col.rgb);
  o.pos = mul(viewProj, w);
  o.misc = float2(i.nrm.w > 0.004 ? 1.0 : 0.0, o.pos.w);
  return o;
}

float4 PSVoxel(VoxelOut i) : SV_Target {
  float3 albedo = i.col;
  float3 emis = emissive.rgb;
  if (i.misc.x > 0.5) { emis += albedo * hemiGround.w; albedo *= fogCol.w; }
  float3 c = lambert(albedo, normalize(i.nrm), i.wpos, emissive.w > 0.5) + emis;
  return float4(finish(c, i.misc.y, objFlags.x), 1.0);
}

float4 VSShadow(VoxelIn i) : SV_Position {
  return mul(shadowVP, mul(world, float4(i.pos, 1.0)));
}

// ---------------------------------------------------------------- sakura petals (instanced)

struct Petal { float3 pos; float scale; float3 rot; float tint; };
StructuredBuffer<Petal> petals : register(t1);

struct PetalOut {
  float4 pos : SV_Position;
  float3 wpos : TEXCOORD0;
  float3 nrm : TEXCOORD1;
  float3 col : TEXCOORD2;
  float depth : TEXCOORD3;
};

PetalOut VSPetal(uint vid : SV_VertexID, uint iid : SV_InstanceID) {
  Petal p = petals[iid];
  float2 c = QUAD[vid] * float2(0.18, 0.14);
  // three.js Euler order 'XYZ': R = Rx * Ry * Rz
  float a = cos(p.rot.x), b = sin(p.rot.x), cy = cos(p.rot.y), d = sin(p.rot.y), e = cos(p.rot.z), f = sin(p.rot.z);
  float3x3 R = {
    cy * e, -cy * f, d,
    a * f + b * e * d, a * e - b * f * d, -b * cy,
    b * f - a * e * d, b * e + a * f * d, a * cy,
  };
  PetalOut o;
  o.wpos = p.pos + mul(R, float3(c, 0.0)) * p.scale;
  o.nrm = float3(d, -b * cy, a * cy);
  o.col = petalCol[(uint)p.tint].rgb;
  o.pos = mul(viewProj, float4(o.wpos, 1.0));
  o.depth = o.pos.w;
  return o;
}

float4 PSPetal(PetalOut i, bool front : SV_IsFrontFace) : SV_Target {
  float3 n = normalize(i.nrm) * (front ? 1.0 : -1.0);
  float3 c = lambert(i.col, n, i.wpos, false) + petalEmissive.rgb;
  return float4(finish(c, i.depth, 1.0), 1.0);
}

// ---------------------------------------------------------------- full-screen passes

struct FsOut { float4 pos : SV_Position; float2 ndc : TEXCOORD0; };

FsOut VSFull(uint vid : SV_VertexID) {
  float2 p = float2((vid << 1) & 2, vid & 2);
  FsOut o;
  o.ndc = p * 2.0 - 1.0;
  o.pos = float4(o.ndc, 1.0, 1.0);
  return o;
}

float hash3(float3 p) {
  p = frac(p * 0.3183099 + 0.1);
  p *= 17.0;
  return frac(p.x * p.y * p.z * (p.x + p.y + p.z));
}

float4 PSSky(FsOut i) : SV_Target {
  float4 far = mul(invViewProj, float4(i.ndc, 1.0, 1.0));
  float3 d = normalize(far.xyz / far.w - camPos.xyz);
  float h = d.y;
  float3 col = lerp(skyHor.rgb, skyZen.rgb, pow(ss(-0.02, 0.85, h), 0.6));
  col = lerp(col, skyGnd.rgb, ss(0.0, -0.3, h));

  float sunVis = skyHor.w;
  float sd = max(dot(d, sunDir.xyz), 0.0);
  col += sunCol.rgb * (pow(sd, 6.0) * 0.22 + pow(sd, 48.0) * 0.45) * sunVis;
  col += sunCol.rgb * ss(0.99935, 0.9996, sd) * 3.0 * sunVis;
  col += sunCol.rgb * sd * sd * exp(-abs(h) * 9.0) * 0.18 * sunVis;

  float moonVis = skyGnd.w;
  float md = dot(d, moonDir.xyz);
  float disc = ss(0.99955, 0.9997, md);
  float3 off = normalize(moonDir.xyz + float3(0.012, 0.006, 0.0));
  float shade = ss(0.99955, 0.9997, dot(d, off));
  col += float3(0.95, 0.97, 1.0) * disc * (1.0 - shade * 0.85) * 1.8 * moonVis;
  col += float3(0.45, 0.55, 0.9) * pow(max(md, 0.0), 300.0) * 0.35 * moonVis;

  float stars = skyZen.w;
  if (stars > 0.001 && h > -0.7) {
    float3 p = d * 230.0;
    float3 id = floor(p);
    float s = hash3(id);
    if (s > 0.972) {
      float3 c = id + 0.5 + (float3(hash3(id + 3.1), hash3(id + 7.7), hash3(id + 1.9)) - 0.5) * 0.6;
      float tw = 0.55 + 0.45 * sin(keyDir.w * (0.8 + s * 3.0) + s * 91.0);
      float b = ss(0.22, 0.0, length(p - c)) * tw * (s - 0.972) / 0.028;
      col += lerp(float3(0.8, 0.85, 1.0), float3(1.0, 0.9, 0.75), hash3(id + 5.0)) * b * 2.0 * stars * lerp(0.25, 1.0, ss(-0.55, 0.1, h));
    }
  }
  return float4(toSrgb(neutral(col)), 1.0);
}

cbuffer Post : register(b3) {
  float4 postInfo;     // xy: 1 / source size, z: FXAA on, w: vignette strength
  float4 postUv;       // xy: scale, zw: offset (output uv -> source uv, for cover / contain)
};

float3 fetch(float2 uv) { return postSrc.SampleLevel(linearSmp, uv, 0).rgb; }

// Scales a rendered view, picture or video frame onto a screen: fit, FXAA (Lottes, console
// variant) and the soft vignette.
float4 PSPost(FsOut i) : SV_Target {
  float2 screen = float2(i.ndc.x * 0.5 + 0.5, 0.5 - i.ndc.y * 0.5);
  float2 uv = screen * postUv.xy + postUv.zw;
  if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) return float4(0.0, 0.0, 0.0, 1.0);
  float2 px = postInfo.xy;
  float3 color = fetch(uv);
  if (postInfo.z > 0.5) {
    const float3 W = float3(0.299, 0.587, 0.114);
    float3 nw = fetch(uv + float2(-1, -1) * px), ne = fetch(uv + float2(1, -1) * px);
    float3 sw = fetch(uv + float2(-1, 1) * px), se = fetch(uv + float2(1, 1) * px);
    float lNW = dot(nw, W), lNE = dot(ne, W), lSW = dot(sw, W), lSE = dot(se, W), lM = dot(color, W);
    float lMin = min(lM, min(min(lNW, lNE), min(lSW, lSE)));
    float lMax = max(lM, max(max(lNW, lNE), max(lSW, lSE)));
    if (lMax - lMin > max(0.0312, lMax * 0.125)) {
      float2 dir = float2(-((lNW + lNE) - (lSW + lSE)), (lNW + lSW) - (lNE + lSE));
      float reduce = max((lNW + lNE + lSW + lSE) * (0.25 / 8.0), 1.0 / 128.0);
      dir = clamp(dir / (min(abs(dir.x), abs(dir.y)) + reduce), -8.0, 8.0) * px;
      float3 a = 0.5 * (fetch(uv + dir * (1.0 / 3.0 - 0.5)) + fetch(uv + dir * (2.0 / 3.0 - 0.5)));
      float3 b = a * 0.5 + 0.25 * (fetch(uv - dir * 0.5) + fetch(uv + dir * 0.5));
      float lB = dot(b, W);
      color = (lB < lMin || lB > lMax) ? a : b;
    }
  }
  // CSS: radial-gradient(ellipse at 50% 45%, transparent 55%, rgba(6, 8, 20, 0.28) 100%)
  float2 q = (screen - float2(0.5, 0.45)) / float2(0.5, 0.45);
  float v = saturate((length(q) / 1.57918 - 0.55) / 0.45) * postInfo.w;
  color = lerp(color, float3(6.0, 8.0, 20.0) / 255.0, v);
  return float4(color, 1.0);
}

// ---------------------------------------------------------------- water & waterfall

struct SurfOut { float4 pos : SV_Position; float3 wpos : TEXCOORD0; float2 uv : TEXCOORD1; };

SurfOut VSSurface(VoxelIn i) {
  SurfOut o;
  o.wpos = mul(world, float4(i.pos, 1.0)).xyz;
  o.pos = mul(viewProj, float4(o.wpos, 1.0));
  o.uv = i.col.rg;
  return o;
}

float wave(float2 p, float t) {
  return sin(p.x * 1.7 + t * 1.3) * 0.5 + sin(p.y * 2.3 - t * 1.1) * 0.5
       + sin((p.x - p.y) * 3.1 + t * 2.0) * 0.25 + sin((p.x * 0.7 + p.y * 1.3) * 5.3 - t * 2.6) * 0.12;
}

float4 PSWater(SurfOut i) : SV_Target {
  float t = keyDir.w;
  float2 p = i.wpos.xz;
  const float e = 0.05;
  float h = wave(p, t);
  float3 n = normalize(float3(-(wave(p + float2(e, 0.0), t) - h) / e * 0.07, 1.0, -(wave(p + float2(0.0, e), t) - h) / e * 0.07));
  float3 V = normalize(camPos.xyz - i.wpos);
  float fres = pow(1.0 - max(dot(n, V), 0.0), 3.0);
  float light = waterDeep.w;
  float3 base = lerp(waterShallow.rgb, waterDeep.rgb, 0.5 + 0.5 * sin(p.x * 0.31) * sin(p.y * 0.37)) * light;
  float3 col = lerp(base, waterSky.rgb, 0.22 + 0.55 * fres);
  float3 R = reflect(-V, n);
  col += waterSunCol.rgb * pow(max(dot(R, waterSunDir.xyz), 0.0), 140.0) * waterSky.w;
  float c = ss(0.7, 1.0, sin(p.x * 2.6 + h * 2.0 + t * 0.8) * sin(p.y * 2.2 - h * 1.5 + t * 0.6));
  col += c * 0.08 * light;
  col += waterGlow.rgb * waterShallow.w * (0.12 + 0.25 * c);
  return float4(toSrgb(neutral(col)), clamp(0.66 + fres * 0.28, 0.0, 0.94));
}

// sine-free hash: stays stable for large inputs (long uptimes)
float hash2(float2 p) {
  float3 p3 = frac(float3(p.xyx) * 0.1031);
  p3 += dot(p3, p3.yzx + 33.33);
  return frac((p3.x + p3.y) * p3.z);
}

float vnoise(float2 p) {
  float2 i = floor(p), f = frac(p);
  f = f * f * (3.0 - 2.0 * f);
  return lerp(lerp(hash2(i), hash2(i + float2(1, 0)), f.x), lerp(hash2(i + float2(0, 1)), hash2(i + float2(1, 1)), f.x), f.y);
}

float4 PSFall(SurfOut i) : SV_Target {
  float t = keyDir.w;
  float v = i.uv.y;
  float s = vnoise(float2(i.uv.x * 6.0, v * 26.0 + t * 4.2)) * 0.6 + vnoise(float2(i.uv.x * 14.0, v * 60.0 + t * 7.0)) * 0.4;
  float edge = ss(0.0, 0.2, i.uv.x) * ss(1.0, 0.8, i.uv.x);
  float fade = ss(0.0, 0.6, v);
  float3 col = lerp(fallCol.rgb, 1.0.xxx, ss(0.42, 0.85, s)) * waterSunDir.w;
  return float4(toSrgb(neutral(col)), edge * fade * (0.5 + 0.5 * s));
}

// ---------------------------------------------------------------- billboards (spray, fireflies, glow halos)

struct Board { float3 pos; float size; float4 col; };
StructuredBuffer<Board> boards : register(t1);

struct BoardOut { float4 pos : SV_Position; float2 uv : TEXCOORD0; float4 col : TEXCOORD1; float depth : TEXCOORD2; };

BoardOut VSBoard(uint vid : SV_VertexID, uint iid : SV_InstanceID) {
  Board b = boards[iid + (uint)objFlags.y]; // SV_InstanceID ignores the start instance in D3D11
  float2 c = QUAD[vid];
  float3 w = b.pos + (camRight.xyz * c.x + camUp.xyz * c.y) * (b.size * 0.5);
  BoardOut o;
  o.pos = mul(viewProj, float4(w, 1.0));
  o.uv = c * 0.5 + 0.5;
  o.col = b.col;
  o.depth = o.pos.w;
  return o;
}

float4 PSPoint(BoardOut i) : SV_Target {
  float a = ss(0.5, 0.0, length(i.uv - 0.5)) * i.col.a;
  return float4(toSrgb(neutral(i.col.rgb)), a);
}

float4 PSGlow(BoardOut i) : SV_Target {
  // the canvas gradient of the web version: 1 -> 0.55 -> 0.14 -> 0 at r = 0, 0.18, 0.45, 1
  float r = length(i.uv - 0.5) * 2.0;
  float g = r < 0.18 ? lerp(1.0, 0.55, r / 0.18) : r < 0.45 ? lerp(0.55, 0.14, (r - 0.18) / 0.27) : lerp(0.14, 0.0, saturate((r - 0.45) / 0.55));
  return float4(finish(i.col.rgb, i.depth, 1.0), g * i.col.a);
}
