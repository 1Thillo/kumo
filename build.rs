// Windows manifest, DLL hardening, precompiled shaders and the Slint UI.

use windows::core::PCSTR;
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCompile, D3DCOMPILE_OPTIMIZATION_LEVEL3};

const SHADERS: [(&str, &str); 14] = [
    ("VSVoxel", "vs_5_0"),
    ("VSShadow", "vs_5_0"),
    ("VSPetal", "vs_5_0"),
    ("VSFull", "vs_5_0"),
    ("VSSurface", "vs_5_0"),
    ("VSBoard", "vs_5_0"),
    ("PSVoxel", "ps_5_0"),
    ("PSPetal", "ps_5_0"),
    ("PSSky", "ps_5_0"),
    ("PSPost", "ps_5_0"),
    ("PSWater", "ps_5_0"),
    ("PSFall", "ps_5_0"),
    ("PSPoint", "ps_5_0"),
    ("PSGlow", "ps_5_0"),
];

/// Shaders become bytecode at build time, so the exe does not need d3dcompiler_47.dll and starts
/// without compiling anything.
fn compile_shaders() {
    println!("cargo:rerun-if-changed=src/shaders.hlsl");
    let src = std::fs::read_to_string("src/shaders.hlsl").unwrap();
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    for (entry, target) in SHADERS {
        let (e, t) = (format!("{entry}\0"), format!("{target}\0"));
        let (mut code, mut errors) = (None, None);
        let r = unsafe {
            D3DCompile(
                src.as_ptr().cast(),
                src.len(),
                PCSTR(b"shaders.hlsl\0".as_ptr()),
                None,
                None,
                PCSTR(e.as_ptr()),
                PCSTR(t.as_ptr()),
                D3DCOMPILE_OPTIMIZATION_LEVEL3,
                0,
                &mut code,
                Some(&mut errors),
            )
        };
        let bytes = |b: &windows::Win32::Graphics::Direct3D::ID3DBlob| unsafe { std::slice::from_raw_parts(b.GetBufferPointer() as *const u8, b.GetBufferSize()).to_vec() };
        if r.is_err() {
            panic!("{entry}: {}", errors.map(|e| String::from_utf8_lossy(&bytes(&e)).into_owned()).unwrap_or_default());
        }
        std::fs::write(out.join(format!("{entry}.cso")), bytes(&code.unwrap())).unwrap();
    }
}

/// One entry of a compiled resource file, identified by numbers.
fn resource(res: &mut Vec<u8>, kind: u16, id: u16, flags: u16, language: u16, data: &[u8]) {
    res.extend((data.len() as u32).to_le_bytes());
    res.extend(32u32.to_le_bytes()); // header size
    for v in [0xFFFF, kind, 0xFFFF, id] {
        res.extend(v.to_le_bytes());
    }
    res.extend(0u32.to_le_bytes());
    res.extend(flags.to_le_bytes());
    res.extend(language.to_le_bytes());
    res.extend([0u8; 8]);
    res.extend(data);
    res.resize(res.len().next_multiple_of(4), 0);
}

/// Windows shows the first icon inside an exe everywhere: on shortcuts, in the taskbar, in the
/// Apps list. The resource file is written here, so the build needs no resource compiler.
fn embed_icon() {
    const RT_ICON: u16 = 3;
    const RT_GROUP_ICON: u16 = 14;
    println!("cargo:rerun-if-changed=assets/kumo.ico");
    let ico = std::fs::read("assets/kumo.ico").unwrap();
    let count = u16::from_le_bytes([ico[4], ico[5]]);
    let mut res = Vec::new();
    resource(&mut res, 0, 0, 0, 0, &[]); // a resource file starts with an empty entry
    let mut group = ico[..6].to_vec();
    for i in 0..count {
        let entry = &ico[6 + i as usize * 16..][..16];
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let id = i + 1;
        resource(&mut res, RT_ICON, id, 0x1010, 0x0409, &ico[offset..offset + size]);
        group.extend(&entry[..12]);
        group.extend(id.to_le_bytes());
    }
    resource(&mut res, RT_GROUP_ICON, 1, 0x1030, 0x0409, &group);
    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("kumo.res");
    std::fs::write(&path, res).unwrap();
    println!("cargo:rustc-link-arg-bins={}", path.display());
}

fn main() {
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("kumo.manifest");
    println!("cargo:rerun-if-changed=kumo.manifest");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());

    // The setup runs from the Downloads folder: imports must only ever come from
    // System32, never from a DLL planted next to the exe (LOAD_LIBRARY_SEARCH_SYSTEM32).
    println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");

    // Only loaded when needed: Media Foundation is missing on Windows N editions (and scenes and
    // pictures do not need it), the panel's accessibility and text DLLs do not belong in the engine.
    for dll in ["mfplat.dll", "mfreadwrite.dll", "uiautomationcore.dll", "dwrite.dll"] {
        println!("cargo:rustc-link-arg-bins=/DELAYLOAD:{dll}");
    }
    println!("cargo:rustc-link-arg-bins=delayimp.lib");

    compile_shaders();
    embed_icon();

    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent".into())
        .with_bundled_translations("lang")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint UI failed to compile");
    println!("cargo:rerun-if-changed=lang");
}
