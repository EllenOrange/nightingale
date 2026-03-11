use std::path::Path;

fn main() {
    let ffmpeg_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let uv_name = if cfg!(windows) { "uv.exe" } else { "uv" };

    let ffmpeg_path = Path::new("vendor-bin").join(ffmpeg_name);
    let uv_path = Path::new("vendor-bin").join(uv_name);

    if !ffmpeg_path.exists() || !uv_path.exists() {
        std::fs::create_dir_all("vendor-bin").ok();
        if !ffmpeg_path.exists() {
            std::fs::write(&ffmpeg_path, b"PLACEHOLDER").unwrap();
            println!("cargo:warning=vendor-bin/{ffmpeg_name} is a placeholder — run the fetch-vendor-bin script or CI to get the real binary");
        }
        if !uv_path.exists() {
            std::fs::write(&uv_path, b"PLACEHOLDER").unwrap();
            println!("cargo:warning=vendor-bin/{uv_name} is a placeholder — run the fetch-vendor-bin script or CI to get the real binary");
        }
    }

    println!("cargo:rerun-if-changed=vendor-bin/{ffmpeg_name}");
    println!("cargo:rerun-if-changed=vendor-bin/{uv_name}");

    set_windows_icon();
}

fn set_windows_icon() {
    let icon_png = Path::new("assets/images/logo_square.png");
    println!("cargo:rerun-if-changed={}", icon_png.display());

    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ico_path = Path::new(&out_dir).join("icon.ico");

    let png_data = std::fs::read(icon_png).expect("Failed to read logo_square.png");

    // ICO can embed PNG data directly (supported since Windows Vista).
    // Format: 6-byte header + 16-byte directory entry + raw PNG bytes.
    let mut ico = Vec::with_capacity(22 + png_data.len());
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type: ICO
    ico.extend_from_slice(&1u16.to_le_bytes()); // image count
    ico.push(0); // width  (0 = 256+, actual size in PNG header)
    ico.push(0); // height (0 = 256+)
    ico.push(0); // color palette count
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // color planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&(png_data.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes()); // data offset (6 + 16)
    ico.extend_from_slice(&png_data);
    std::fs::write(&ico_path, &ico).expect("Failed to write icon.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    res.compile().expect("Failed to compile Windows resource");
}
