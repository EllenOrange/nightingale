use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;

pub fn silent_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[derive(Debug, Clone)]
pub struct BootstrapProgress {
    pub step: &'static str,
    pub detail: String,
    pub done: bool,
    pub error: Option<String>,
}

fn nightingale_dir() -> PathBuf {
    dirs::home_dir()
        .expect("could not find home directory")
        .join(".nightingale")
}

pub fn vendor_dir() -> PathBuf {
    nightingale_dir().join("vendor")
}

pub fn models_dir() -> PathBuf {
    nightingale_dir().join("models")
}

pub fn ffmpeg_path() -> PathBuf {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    vendor_dir().join(name)
}

pub fn python_path() -> PathBuf {
    if cfg!(windows) {
        vendor_dir().join("venv").join("Scripts").join("python.exe")
    } else {
        vendor_dir().join("venv").join("bin").join("python")
    }
}

pub fn analyzer_dir() -> PathBuf {
    vendor_dir().join("analyzer")
}

fn uv_path() -> PathBuf {
    let name = if cfg!(windows) { "uv.exe" } else { "uv" };
    vendor_dir().join(name)
}

fn ready_marker() -> PathBuf {
    vendor_dir().join(".ready")
}

pub fn is_ready() -> bool {
    ready_marker().is_file()
        && ffmpeg_path().is_file()
        && python_path().is_file()
        && analyzer_dir().join("analyze.py").is_file()
}

pub fn videos_dir() -> PathBuf {
    nightingale_dir().join("videos")
}

pub fn reset() {
    let marker = ready_marker();
    if marker.is_file() {
        let _ = std::fs::remove_file(marker);
    }
}

pub fn clear_videos() {
    let base = videos_dir();
    if !base.is_dir() {
        return;
    }
    for entry in std::fs::read_dir(&base).into_iter().flatten().flatten() {
        let flavor_dir = entry.path();
        if !flavor_dir.is_dir() {
            continue;
        }
        let mut mp4s: Vec<_> = std::fs::read_dir(&flavor_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "mp4"))
            .collect();
        mp4s.sort();
        for path in mp4s.into_iter().skip(1) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

pub fn clearable_video_bytes() -> u64 {
    let base = videos_dir();
    if !base.is_dir() {
        return 0;
    }
    let mut total: u64 = 0;
    for entry in std::fs::read_dir(&base).into_iter().flatten().flatten() {
        let flavor_dir = entry.path();
        if !flavor_dir.is_dir() {
            continue;
        }
        let mut mp4s: Vec<_> = std::fs::read_dir(&flavor_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "mp4"))
            .collect();
        mp4s.sort();
        for path in mp4s.into_iter().skip(1) {
            total += path.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    total
}

pub fn clear_models() {
    let dir = models_dir();
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

fn send(tx: &mpsc::Sender<BootstrapProgress>, step: &'static str, detail: impl Into<String>) {
    let _ = tx.send(BootstrapProgress {
        step,
        detail: detail.into(),
        done: false,
        error: None,
    });
}

fn send_done(tx: &mpsc::Sender<BootstrapProgress>) {
    let _ = tx.send(BootstrapProgress {
        step: "Done",
        detail: "Setup complete!".into(),
        done: true,
        error: None,
    });
}

fn send_error(tx: &mpsc::Sender<BootstrapProgress>, msg: impl Into<String>) {
    let _ = tx.send(BootstrapProgress {
        step: "Error",
        detail: String::new(),
        done: true,
        error: Some(msg.into()),
    });
}

pub fn run_bootstrap(tx: mpsc::Sender<BootstrapProgress>) {
    let vdir = vendor_dir();
    let _ = std::fs::create_dir_all(&vdir);
    let _ = std::fs::create_dir_all(models_dir().join("torch"));
    let _ = std::fs::create_dir_all(models_dir().join("huggingface"));

    if let Err(e) = step_download_ffmpeg(&tx) {
        send_error(&tx, format!("Failed to download ffmpeg: {e}"));
        return;
    }

    if let Err(e) = step_download_uv(&tx) {
        send_error(&tx, format!("Failed to download uv: {e}"));
        return;
    }

    if let Err(e) = step_install_python(&tx) {
        send_error(&tx, format!("Failed to install Python: {e}"));
        return;
    }

    if let Err(e) = step_create_venv(&tx) {
        send_error(&tx, format!("Failed to create venv: {e}"));
        return;
    }

    if let Err(e) = step_install_packages(&tx) {
        send_error(&tx, format!("Failed to install packages: {e}"));
        return;
    }

    if let Err(e) = step_extract_scripts(&tx) {
        send_error(&tx, format!("Failed to extract analyzer scripts: {e}"));
        return;
    }

    step_prefetch_videos(&tx);

    if let Err(e) = std::fs::write(ready_marker(), "ok") {
        send_error(&tx, format!("Failed to write ready marker: {e}"));
        return;
    }

    send_done(&tx);
}

// ─── Step 1: Download ffmpeg ─────────────────────────────────────────

fn ffmpeg_download_url() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"),
        ("linux", "aarch64") => Ok("https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz"),
        ("macos", "aarch64") => Ok("https://www.osxexperts.net/ffmpeg7arm.zip"),
        ("macos", "x86_64") => Ok("https://www.osxexperts.net/ffmpeg7intel.zip"),
        ("windows", "x86_64") => Ok("https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"),
        (os, arch) => Err(format!("Unsupported platform for ffmpeg: {os}-{arch}")),
    }
}

fn step_download_ffmpeg(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let dest = ffmpeg_path();
    if dest.is_file() {
        send(tx, "ffmpeg", "Already installed");
        return Ok(());
    }

    let url = ffmpeg_download_url()?;
    send(tx, "ffmpeg", "Downloading ffmpeg...");

    let tmp_dir = vendor_dir().join("_tmp_ffmpeg");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let ext = if url.ends_with(".tar.xz") { "tar.xz" } else { "zip" };
    let archive = tmp_dir.join(format!("ffmpeg.{ext}"));

    let result: Result<(), String> = (|| {
        download_to_file(url, &archive)?;
        send(tx, "ffmpeg", "Extracting ffmpeg...");
        extract_archive(&archive, &tmp_dir)?;

        let binary_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
        let found = find_file_in(&tmp_dir, binary_name)
            .ok_or_else(|| format!("Could not find {binary_name} in downloaded archive"))?;

        std::fs::copy(&found, &dest)
            .map_err(|e| format!("Failed to copy ffmpeg: {e}"))?;
        mark_executable(&dest)?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&tmp_dir);
    result?;

    send(tx, "ffmpeg", "ffmpeg ready");
    Ok(())
}

// ─── Step 2: Download uv ────────────────────────────────────────────

fn uv_download_url() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-unknown-linux-gnu.tar.gz"),
        ("linux", "aarch64") => Ok("https://github.com/astral-sh/uv/releases/latest/download/uv-aarch64-unknown-linux-gnu.tar.gz"),
        ("macos", "aarch64") => Ok("https://github.com/astral-sh/uv/releases/latest/download/uv-aarch64-apple-darwin.tar.gz"),
        ("macos", "x86_64") => Ok("https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-apple-darwin.tar.gz"),
        ("windows", "x86_64") => Ok("https://github.com/astral-sh/uv/releases/latest/download/uv-x86_64-pc-windows-msvc.zip"),
        (os, arch) => Err(format!("Unsupported platform for uv: {os}-{arch}")),
    }
}

fn step_download_uv(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let dest = uv_path();
    if dest.is_file() {
        send(tx, "uv", "Already installed");
        return Ok(());
    }

    let url = uv_download_url()?;
    send(tx, "uv", "Downloading uv...");

    let tmp_dir = vendor_dir().join("_tmp_uv");
    let _ = std::fs::create_dir_all(&tmp_dir);

    let ext = if url.ends_with(".zip") { "zip" } else { "tar.gz" };
    let archive = tmp_dir.join(format!("uv.{ext}"));

    let result: Result<(), String> = (|| {
        download_to_file(url, &archive)?;
        send(tx, "uv", "Extracting uv...");
        extract_archive(&archive, &tmp_dir)?;

        let binary_name = if cfg!(windows) { "uv.exe" } else { "uv" };
        let found = find_file_in(&tmp_dir, binary_name)
            .ok_or_else(|| format!("Could not find {binary_name} in downloaded archive"))?;

        std::fs::copy(&found, &dest)
            .map_err(|e| format!("Failed to copy uv: {e}"))?;
        mark_executable(&dest)?;
        Ok(())
    })();

    let _ = std::fs::remove_dir_all(&tmp_dir);
    result?;

    send(tx, "uv", "uv ready");
    Ok(())
}

// ─── Download helpers ───────────────────────────────────────────────

fn download_to_file(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let resp = ureq::get(url).call().map_err(|e| e.to_string())?;
    let mut body = resp.into_body();
    let mut reader = body.as_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
    Ok(())
}

fn extract_archive(archive: &std::path::Path, dest_dir: &std::path::Path) -> Result<(), String> {
    let name = archive.to_string_lossy();

    let output = if name.ends_with(".tar.xz") {
        silent_command("tar")
            .arg("-xJf").arg(archive)
            .arg("-C").arg(dest_dir)
            .output()
    } else if name.ends_with(".tar.gz") {
        silent_command("tar")
            .arg("-xzf").arg(archive)
            .arg("-C").arg(dest_dir)
            .output()
    } else if name.ends_with(".zip") {
        #[cfg(windows)]
        {
            silent_command("tar")
                .arg("-xf").arg(archive)
                .arg("-C").arg(dest_dir)
                .output()
        }
        #[cfg(not(windows))]
        {
            silent_command("unzip")
                .arg("-o").arg(archive)
                .arg("-d").arg(dest_dir)
                .output()
        }
    } else {
        return Err(format!("Unknown archive format: {name}"));
    };

    let output = output.map_err(|e| format!("Failed to run extraction command: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Extraction failed: {stderr}"));
    }
    Ok(())
}

fn find_file_in(dir: &std::path::Path, name: &str) -> Option<PathBuf> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .find(|e| e.file_type().is_file() && e.file_name().to_string_lossy() == name)
        .map(|e| e.into_path())
}

fn mark_executable(_path: &std::path::Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {e}"))?;
    }
    Ok(())
}

// ─── Step 3: Install Python via uv ──────────────────────────────────

fn step_install_python(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let python_dir = vendor_dir().join("python");
    if python_dir.is_dir() && has_python_in(&python_dir) {
        send(tx, "Python", "Already installed");
        return Ok(());
    }

    send(tx, "Python", "Installing Python 3.10...");

    let output = silent_command(uv_path())
        .args(["python", "install", "3.10", "--install-dir"])
        .arg(&python_dir)
        .output()
        .map_err(|e| format!("Failed to run uv: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("uv python install failed: {stderr}"));
    }

    send(tx, "Python", "Python 3.10 installed");
    Ok(())
}

fn has_python_in(dir: &PathBuf) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let target = if cfg!(windows) { "python.exe" } else { "python3.10" };
    for entry in walkdir::WalkDir::new(dir).max_depth(5).into_iter().flatten() {
        if entry.file_name().to_string_lossy() == target {
            return true;
        }
    }
    false
}

// ─── Step 4: Create venv ─────────────────────────────────────────────

fn find_installed_python() -> Option<PathBuf> {
    let python_dir = vendor_dir().join("python");
    let target = if cfg!(windows) { "python.exe" } else { "python3.10" };
    for entry in walkdir::WalkDir::new(&python_dir).max_depth(5).into_iter().flatten() {
        if entry.file_name().to_string_lossy() == target {
            return Some(entry.into_path());
        }
    }
    None
}

fn step_create_venv(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let venv_dir = vendor_dir().join("venv");
    if python_path().is_file() {
        send(tx, "Venv", "Already created");
        return Ok(());
    }

    send(tx, "Venv", "Creating Python virtual environment...");

    let installed_python = find_installed_python()
        .ok_or("Could not find installed Python — run python install first")?;

    let output = silent_command(uv_path())
        .args(["venv"])
        .arg(&venv_dir)
        .arg("--python")
        .arg(&installed_python)
        .output()
        .map_err(|e| format!("Failed to run uv venv: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("uv venv failed: {stderr}"));
    }

    send(tx, "Venv", "Virtual environment created");
    Ok(())
}

// ─── Step 5: Install packages ────────────────────────────────────────

fn detect_gpu() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "mps";
    }

    #[cfg(not(target_os = "macos"))]
    {
        if silent_command("nvidia-smi")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
        {
            "cuda"
        } else {
            "cpu"
        }
    }
}

fn step_install_packages(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let gpu = detect_gpu();
    send(tx, "Packages", format!("Detected compute: {gpu}. Installing PyTorch..."));

    let uv = uv_path();
    let py = python_path();

    let mut torch_args: Vec<&str> = vec![
        "pip", "install",
        "torch>=2.0.0", "torchaudio>=2.0.0",
        "--python",
    ];
    let py_str = py.to_string_lossy().to_string();
    torch_args.push(&py_str);

    let index_url = match gpu {
        "cuda" => Some("https://download.pytorch.org/whl/cu121"),
        "cpu" => Some("https://download.pytorch.org/whl/cpu"),
        _ => None,
    };
    if let Some(url) = index_url {
        torch_args.extend(["--index-url", url]);
    }

    let output = silent_command(&uv)
        .args(&torch_args)
        .output()
        .map_err(|e| format!("Failed to run uv pip install torch: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PyTorch install failed: {stderr}"));
    }

    let audio_sep_pkg = if gpu == "cuda" {
        "audio-separator[gpu]>=0.25"
    } else {
        "audio-separator>=0.25"
    };
    send(tx, "Packages", "Installing Demucs, WhisperX and audio-separator...");

    let cython_out = silent_command(&uv)
        .args(["pip", "install", "cython", "setuptools", "--python"])
        .arg(&py)
        .output()
        .map_err(|e| format!("Failed to install build deps: {e}"))?;
    if !cython_out.status.success() {
        let stderr = String::from_utf8_lossy(&cython_out.stderr);
        return Err(format!("Build deps install failed: {stderr}"));
    }

    let output = silent_command(&uv)
        .args([
            "pip", "install",
            "demucs>=4.0.0", "whisperx>=3.3.0", "soundfile",
            "huggingface_hub>=0.27.0",
            audio_sep_pkg,
            "--python",
        ])
        .arg(&py)
        .output()
        .map_err(|e| format!("Failed to run uv pip install: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Package install failed: {stderr}"));
    }

    send(tx, "Packages", "All packages installed");
    Ok(())
}

// ─── Step 6: Extract analyzer scripts ────────────────────────────────

fn step_extract_scripts(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    send(tx, "Scripts", "Extracting analyzer scripts...");
    crate::vendor_scripts::write_scripts(&analyzer_dir())
        .map_err(|e| format!("Failed to write scripts: {e}"))?;
    send(tx, "Scripts", "Analyzer scripts extracted");
    Ok(())
}

// ─── Step 7: Pre-fetch one video background per flavor ───────────────

fn step_prefetch_videos(tx: &mpsc::Sender<BootstrapProgress>) {
    send(tx, "Videos", "Pre-downloading video backgrounds...");
    crate::player::video_bg::prefetch_one_per_flavor(|detail| {
        send(tx, "Videos", detail);
    });
    send(tx, "Videos", "Video backgrounds ready");
}
