//! On Windows, gives xc-circuit.exe its icon and version information, so
//! Explorer shows the icon and Task Manager the name "XC™ Circuit" instead of
//! the file name.

include!("src/icon.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ico = out.join("xc-circuit.ico");
    std::fs::write(&ico, ico_file(&[16, 20, 24, 32, 40, 48, 64, 96, 128, 256])).expect("writing the icon");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico.to_str().expect("icon path"))
        .set("FileDescription", "XC™ Circuit")
        .set("ProductName", "XC™ Circuit")
        .set("CompanyName", "XC Team")
        .set("InternalName", "xc-circuit")
        .set("OriginalFilename", "xc-circuit.exe")
        .set("LegalCopyright", "XC Team");
    if let Err(e) = res.compile() {
        // A build without the Windows resource compiler still works, only
        // without the icon and name.
        println!("cargo:warning=no icon and version info in the .exe: {e}");
    }
}

/// An .ico holding the icon at each size, as PNG images.
fn ico_file(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png_bytes(&icon_rgba(s), s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&size, png) in sizes.iter().zip(&images) {
        // 256 is written as 0 in the one-byte width and height.
        let side = if size >= 256 { 0 } else { size as u8 };
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in images {
        out.extend_from_slice(&png);
    }
    out
}

fn png_bytes(rgba: &[u8], size: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
    writer.finish().expect("png end");
    bytes
}
