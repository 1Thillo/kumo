//! `.kumo` scene files: a JSON manifest plus one vertex/index blob (written by the bake tool).
//!
//! Layout: "KUMO" | u32 version | u32 jsonLen | json | u32 binLen | bin
//! The bin holds every vertex (20 bytes, see [`VERTEX_SIZE`]) followed by u32 indices.

use serde::Deserialize;

pub const VERTEX_SIZE: usize = 20;

pub type V3 = [f32; 3];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub title: String,
    pub camera: Camera,
    pub fog: Fog,
    pub shadow: Shadow,
    pub sky: Sky,
    pub objects: Vec<Object>,
    pub swimmers: Vec<Swimmer>,
    pub water: Water,
    pub waterfall: Waterfall,
    pub petals: Petals,
    pub ground: Ground,
    pub fireflies: Fireflies,
    pub glows: Vec<Glow>,
    pub lights: Vec<Light>,
    pub meshes: Vec<Mesh>,
}

#[derive(Deserialize)]
pub struct Camera {
    pub target: V3,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub angle: f32,
    pub elev: f32,
    pub fit: [f32; 2],
    #[serde(rename = "orbitSpeed")]
    pub orbit_speed: f32,
}

#[derive(Deserialize)]
pub struct Fog {
    pub near: f32,
    pub far: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    pub size: u32,
    pub extent: f32,
    pub near: f32,
    pub far: f32,
    pub target: V3,
    pub distance: f32,
    pub bias: f32,
    pub normal_bias: f32,
}

#[derive(Deserialize)]
pub struct Sky {
    pub keys: Vec<SkyKey>,
    pub sun: Body,
    pub moon: Body,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct SkyKey {
    pub h: f32,
    pub zen: V3,
    pub hor: V3,
    pub gnd: V3,
    pub key_c: V3,
    pub hemi_s: V3,
    pub hemi_g: V3,
    pub key: f32,
    pub hemi_i: f32,
    pub glow: f32,
    pub stars: f32,
    pub exp: f32,
}

#[derive(Deserialize)]
pub struct Body {
    #[serde(default)]
    pub rise: f32,
    #[serde(default)]
    pub set: f32,
    pub tilt: f32,
    pub yaw: f32,
}

#[derive(Deserialize)]
pub struct Object {
    pub mesh: usize,
    pub cast: bool,
    pub receive: bool,
    pub emissive: V3,
    #[serde(default)]
    pub pos: Option<V3>,
    #[serde(default)]
    pub scale: Option<V3>,
    #[serde(default, rename = "rotY")]
    pub rot_y: f32,
    #[serde(default)]
    pub bob: Option<Bob>,
    #[serde(default)]
    pub orbit: Option<Orbit>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bob {
    pub amp: f32,
    pub speed: f32,
    pub phase: f32,
    pub rot_amp: f32,
    pub rot_speed: f32,
}

#[derive(Deserialize)]
pub struct Orbit {
    pub angle: f32,
    pub radius: f32,
    pub y: f32,
    pub speed: f32,
    pub wobble: f32,
}

#[derive(Deserialize)]
pub struct Swimmer {
    pub mesh: usize,
    pub scale: V3,
    pub center: [f32; 2],
    pub th: f32,
    pub w: f32,
    pub rx: f32,
    pub rz: f32,
    pub ph: f32,
    pub ox: f32,
    pub oz: f32,
    pub y: f32,
}

#[derive(Deserialize)]
pub struct Water {
    pub mesh: usize,
    pub deep: V3,
    pub shallow: V3,
    pub glow: V3,
}

#[derive(Deserialize)]
pub struct Waterfall {
    pub mesh: usize,
    pub x: f32,
    pub y: f32,
    pub z0: f32,
    pub z1: f32,
    pub color: V3,
    pub spray: V3,
}

#[derive(Deserialize)]
pub struct Petals {
    pub count: u32,
    pub max: u32,
    pub emissive: V3,
    pub colors: Vec<V3>,
    pub spawns: Vec<V3>,
}

#[derive(Deserialize)]
pub struct Ground {
    pub half: i32,
    pub heights: Vec<f32>,
}

#[derive(Deserialize)]
pub struct Fireflies {
    pub color: V3,
    pub items: Vec<[f32; 5]>,
}

#[derive(Deserialize)]
pub struct Glow {
    pub pos: V3,
    pub size: f32,
    pub color: V3,
    pub strength: f32,
    #[serde(default)]
    pub follow: Option<usize>,
    #[serde(default)]
    pub local: Option<V3>,
}

#[derive(Deserialize)]
pub struct Light {
    pub pos: V3,
    pub color: V3,
    pub intensity: f32,
    pub distance: f32,
    pub decay: f32,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Mesh {
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub first_index: u32,
    pub index_count: u32,
}

pub struct Scene<'a> {
    pub manifest: Manifest,
    pub vertices: &'a [u8],
    pub indices: &'a [u8],
}

impl Manifest {
    /// Everything the renderer and the simulation index without looking: a wrong number in a
    /// hand-made file has to end here, with a message, and not in a crash at every start.
    fn validate(&self, vertices: usize, indices: usize) -> Result<(), String> {
        let check = |ok: bool, what: &str| if ok { Ok(()) } else { Err(format!("invalid scene: {what}")) };
        let meshes = self.meshes.len();
        check(self.meshes.iter().all(|m| m.first_vertex as usize + m.vertex_count as usize <= vertices), "mesh outside the vertex data")?;
        check(self.meshes.iter().all(|m| m.first_index as usize + m.index_count as usize <= indices), "mesh outside the index data")?;
        check(self.water.mesh < meshes && self.waterfall.mesh < meshes, "water without a mesh")?;
        check(self.objects.iter().all(|o| o.mesh < meshes), "object without a mesh")?;
        check(self.swimmers.iter().all(|s| s.mesh < meshes), "swimmer without a mesh")?;
        let draws = self.objects.len() + self.swimmers.len();
        check(self.glows.iter().all(|g| g.follow.is_none_or(|f| f < draws)), "glow follows an unknown object")?;
        check(self.petals.max <= 20_000, "too many petals")?;
        check(self.petals.max == 0 || !self.petals.spawns.is_empty(), "petals without a spawn point")?;
        check((1..=4096).contains(&self.ground.half), "ground size")?;
        check(self.ground.heights.len() == (self.ground.half as usize * 2).pow(2), "ground height map size")?;
        check(!self.sky.keys.is_empty(), "sky without key frames")?;
        check((16..=8192).contains(&self.shadow.size), "shadow map size")
    }
}

fn u32_at(b: &[u8], o: usize) -> Result<u32, String> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or_else(|| "truncated file".into())
}

pub fn parse(bytes: &[u8]) -> Result<Scene<'_>, String> {
    if bytes.get(0..4) != Some(b"KUMO") {
        return Err("not a .kumo scene".into());
    }
    let version = u32_at(bytes, 4)?;
    if version != 1 {
        return Err(format!("unsupported scene version {version}"));
    }
    let json_len = u32_at(bytes, 8)? as usize;
    let json = bytes.get(12..12 + json_len).ok_or("truncated manifest")?;
    let manifest: Manifest = serde_json::from_slice(json).map_err(|e| format!("manifest: {e}"))?;
    let bin_start = 12 + json_len + 4;
    let bin_len = u32_at(bytes, 12 + json_len)? as usize;
    let bin = bytes.get(bin_start..bin_start + bin_len).ok_or("truncated geometry")?;
    let vcount: usize = manifest.meshes.iter().map(|m| m.vertex_count as usize).sum();
    let split = vcount * VERTEX_SIZE;
    if split > bin.len() {
        return Err("geometry size mismatch".into());
    }
    let (vertices, indices) = bin.split_at(split);
    manifest.validate(vertices.len() / VERTEX_SIZE, indices.len() / 4)?;
    Ok(Scene { manifest, vertices, indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_scene_is_consistent() {
        let s = parse(crate::library::PAGODA_SCENE).unwrap();
        let m = &s.manifest;
        assert_eq!(s.vertices.len() % VERTEX_SIZE, 0);
        assert_eq!(s.indices.len() % 4, 0);
        let (verts, idx) = ((s.vertices.len() / VERTEX_SIZE) as u32, (s.indices.len() / 4) as u32);
        for mesh in &m.meshes {
            assert!(mesh.first_vertex + mesh.vertex_count <= verts);
            assert!(mesh.first_index + mesh.index_count <= idx);
        }
        assert!(m.objects.iter().all(|o| o.mesh < m.meshes.len()));
        assert_eq!(m.ground.heights.len(), (m.ground.half * 2 * m.ground.half * 2) as usize);
        assert!(!m.petals.spawns.is_empty());
        assert_eq!(m.petals.colors.len(), 5);
        assert!(m.lights.len() <= 4);
    }

    /// The built-in scene with one thing broken.
    fn broken(change: impl Fn(&mut serde_json::Value)) -> Result<(), String> {
        let bytes = crate::library::PAGODA_SCENE;
        let len = u32_at(bytes, 8).unwrap() as usize;
        let mut json: serde_json::Value = serde_json::from_slice(&bytes[12..12 + len]).unwrap();
        change(&mut json);
        let text = serde_json::to_vec(&json).unwrap();
        let mut file = bytes[..8].to_vec();
        file.extend_from_slice(&(text.len() as u32).to_le_bytes());
        file.extend_from_slice(&text);
        file.extend_from_slice(&bytes[12 + len..]);
        parse(&file).map(|_| ())
    }

    #[test]
    fn rejects_scenes_that_would_crash_the_engine() {
        assert_eq!(broken(|_| {}), Ok(()));
        assert!(broken(|j| j["water"]["mesh"] = 99999.into()).is_err());
        assert!(broken(|j| j["objects"][0]["mesh"] = 99999.into()).is_err());
        assert!(broken(|j| j["glows"][0]["follow"] = 99999.into()).is_err());
        assert!(broken(|j| j["petals"]["spawns"] = serde_json::json!([])).is_err());
        assert!(broken(|j| j["ground"]["heights"] = serde_json::json!([0.0])).is_err());
        assert!(broken(|j| j["sky"]["keys"] = serde_json::json!([])).is_err());
        assert!(broken(|j| j["shadow"]["size"] = 0.into()).is_err());
    }

    #[test]
    fn rejects_other_files() {
        assert!(parse(b"not a scene").is_err());
        assert!(parse(b"KUMO").is_err());
        assert!(parse(&[]).is_err());
        // right magic, absurd manifest length
        let mut b = b"KUMO".to_vec();
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse(&b).is_err());
    }
}
