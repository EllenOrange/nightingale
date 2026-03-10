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
}
