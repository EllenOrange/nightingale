use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;

#[cfg(windows)]
const EMBEDDED_FFMPEG: &[u8] = include_bytes!("../vendor-bin/ffmpeg.exe");
#[cfg(not(windows))]
const EMBEDDED_FFMPEG: &[u8] = include_bytes!("../vendor-bin/ffmpeg");

#[cfg(windows)]
const EMBEDDED_UV: &[u8] = include_bytes!("../vendor-bin/uv.exe");
#[cfg(not(windows))]
const EMBEDDED_UV: &[u8] = include_bytes!("../vendor-bin/uv");

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

pub fn reset() {
    let marker = ready_marker();
    if marker.is_file() {
        let _ = std::fs::remove_file(marker);
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

    if let Err(e) = step_extract_ffmpeg(&tx) {
        send_error(&tx, format!("Failed to extract ffmpeg: {e}"));
        return;
    }

    if let Err(e) = step_extract_uv(&tx) {
        send_error(&tx, format!("Failed to extract uv: {e}"));
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

// ─── Step 1: Extract bundled ffmpeg ──────────────────────────────────

fn step_extract_ffmpeg(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let dest = ffmpeg_path();
    if dest.is_file() {
        send(tx, "ffmpeg", "Already installed");
        return Ok(());
    }

    send(tx, "ffmpeg", "Extracting bundled ffmpeg...");
    write_binary(EMBEDDED_FFMPEG, &dest)?;
    send(tx, "ffmpeg", "ffmpeg ready");
    Ok(())
}

// ─── Step 2: Extract bundled uv ─────────────────────────────────────

fn step_extract_uv(tx: &mpsc::Sender<BootstrapProgress>) -> Result<(), String> {
    let dest = uv_path();
    if dest.is_file() {
        send(tx, "uv", "Already installed");
        return Ok(());
    }

    send(tx, "uv", "Extracting bundled uv...");
    write_binary(EMBEDDED_UV, &dest)?;
    send(tx, "uv", "uv ready");
    Ok(())
}

fn write_binary(data: &[u8], dest: &PathBuf) -> Result<(), String> {
    std::fs::write(dest, data).map_err(|e| format!("Failed to write {}: {e}", dest.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
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

    send(tx, "Python", "Installing Python 3.11...");

    let output = Command::new(uv_path())
        .args(["python", "install", "3.11", "--install-dir"])
        .arg(&python_dir)
        .output()
        .map_err(|e| format!("Failed to run uv: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("uv python install failed: {stderr}"));
    }

    send(tx, "Python", "Python 3.11 installed");
    Ok(())
}

fn has_python_in(dir: &PathBuf) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let target = if cfg!(windows) { "python.exe" } else { "python3.11" };
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
    let target = if cfg!(windows) { "python.exe" } else { "python3.11" };
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

    let output = Command::new(uv_path())
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
        if Command::new("nvidia-smi")
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

    let output = Command::new(&uv)
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

    let output = Command::new(&uv)
        .args([
            "pip", "install",
            "demucs>=4.0.0", "whisperx>=3.3.0", "soundfile",
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
