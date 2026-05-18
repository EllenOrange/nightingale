use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::commands::{ApiError, CmdResult};
use crate::events::EventBus;

/// Matches `client/src-tauri/src/vendor.rs::SetupStep` so the existing setup
/// dialog renders unchanged under the `listen("setup-progress", ...)` shim.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SetupStep {
    MigrateData,
    ClearVendor,
    Ffmpeg,
    Uv,
    Python,
    Venv,
    Dependencies,
    ExtractScripts,
    Videos,
    Finish,
}

#[derive(Debug, Clone, Serialize)]
struct SetupProgress {
    step: SetupStep,
    percent: usize,
    action: String,
}

fn emit(events: &EventBus, step: SetupStep, percent: usize, action: impl Into<String>) {
    events.emit(
        "setup-progress",
        &SetupProgress {
            step,
            percent,
            action: action.into(),
        },
    );
}

fn resolve_data_path_input(input: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|e| format!("Failed to resolve data path: {e}"))
    }
}

fn same_path(lhs: &Path, rhs: &Path) -> bool {
    match (
        std::fs::canonicalize(lhs).ok(),
        std::fs::canonicalize(rhs).ok(),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => lhs == rhs,
    }
}

fn run_pipeline(events: &EventBus, data_path: Option<String>) -> Result<(), String> {
    let mut cleared_vendor = false;

    /*
     * Honour an operator-supplied data folder the same way Tauri does: the
     * picker in the web setup wizard sends a server-side absolute path (the
     * `selectFolderRaw` web fallback prompts for one), so the user can point
     * Nightingale at any directory the container can see — useful when the
     * caller mounts several volumes and wants to choose between them.
     */
    if let Some(raw) = data_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let target = resolve_data_path_input(raw)?;
        let current = app_core::nightingale_dir();
        if !same_path(&current, &target) {
            emit(
                events,
                SetupStep::ClearVendor,
                6,
                "Clearing vendor folder before migration...",
            );
            app_core::clear_vendor_dir()?;
            cleared_vendor = true;

            emit(events, SetupStep::MigrateData, 12, "Migrating app data...");
            let new_path = app_core::change_app_data_path(target)?;
            emit(
                events,
                SetupStep::MigrateData,
                18,
                format!("Data migrated to {}", new_path.display()),
            );
        }
    }

    if !cleared_vendor {
        emit(
            events,
            SetupStep::ClearVendor,
            14,
            "Clearing vendor folder...",
        );
        app_core::clear_vendor_dir()?;
    }

    emit(events, SetupStep::Ffmpeg, 24, "Downloading ffmpeg...");
    app_core::step_download_ffmpeg()?;

    emit(events, SetupStep::Uv, 34, "Downloading uv...");
    app_core::step_download_uv()?;

    emit(
        events,
        SetupStep::Python,
        46,
        "Installing python3.10 via uv...",
    );
    app_core::step_install_python()?;

    emit(events, SetupStep::Venv, 58, "Setting up .venv...");
    app_core::step_create_venv()?;

    emit(
        events,
        SetupStep::Dependencies,
        70,
        "Installing python dependencies...",
    );
    app_core::step_install_packages()?;

    emit(
        events,
        SetupStep::ExtractScripts,
        80,
        "Extracting analyzer scripts...",
    );
    app_core::step_extract_scripts()?;

    emit(
        events,
        SetupStep::Videos,
        90,
        "Pre-downloading video backgrounds...",
    );
    {
        let events = events.clone();
        app_core::prefetch_one_per_flavor(move |detail| {
            emit(&events, SetupStep::Videos, 90, detail);
        });
    }

    app_core::mark_ready()?;

    emit(events, SetupStep::Finish, 100, "Done");
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TriggerSetupArgs {
    #[serde(default)]
    data_path: Option<String>,
}

pub fn trigger_setup(events: Arc<EventBus>, payload: Value) -> CmdResult {
    let args: TriggerSetupArgs = if payload.is_null() {
        TriggerSetupArgs { data_path: None }
    } else {
        serde_json::from_value(payload)
            .map_err(|e| ApiError::bad_request(format!("invalid trigger_setup args: {e}")))?
    };

    let events_clone = events.clone();
    std::thread::spawn(move || {
        if let Err(e) = run_pipeline(&events_clone, args.data_path) {
            events_clone.emit_value("setup-error", serde_json::Value::String(e));
        }
    });
    Ok(Value::Null)
}
