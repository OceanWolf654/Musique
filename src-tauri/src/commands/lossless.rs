use std::path::Path;
use tauri::{AppHandle, Manager};

use crate::errors::AppError;
use crate::lossless::{
    devices::list_output_devices,
    get_capabilities,
    library::{get_all_local_tracks, scan_directory},
    probe_file,
    AudioDeviceFormatInfo, AudioFormatInfo, LocalLosslessTrack, LosslessCapabilitiesReport,
    LosslessPlayer,
};
use crate::state::AppState;

/// Initialize the lossless player backend if not already active.
pub async fn ensure_lossless(app: &AppHandle) -> Result<(), AppError> {
    let s = app.state::<AppState>();
    let lossless = s.lossless.clone();
    let media_tx = s.media_tx.clone();
    let db = s.db.clone();
    drop(s);

    let mut guard = lossless.lock().await;
    if guard.is_none() {
        let vol_row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'player_volume'")
            .fetch_optional(&db)
            .await
            .ok()
            .flatten();
        let vol = vol_row
            .and_then(|(v,)| v.parse::<f64>().ok())
            .map(|v| v / 100.0)
            .unwrap_or(0.8);

        let muted_row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'player_muted'")
            .fetch_optional(&db)
            .await
            .ok()
            .flatten();
        let muted = muted_row.map(|(v,)| v == "1").unwrap_or(false);

        let player = LosslessPlayer::new(
            app.clone(),
            crate::playback::SharedVolume::new(vol, muted),
            media_tx,
        );
        *guard = Some(player);
    }
    Ok(())
}

/// Retrieve the active audio format, bit depth, sample rate, and signal path.
#[tauri::command]
pub async fn get_current_audio_format(app: AppHandle) -> Result<Option<AudioFormatInfo>, AppError> {
    let s = app.state::<AppState>();
    let guard = s.current_format.lock().await;
    Ok(guard.clone())
}

/// Retrieve full transparent capabilities report detailing audio support and Spotify limits.
#[tauri::command]
pub async fn get_lossless_capabilities() -> Result<LosslessCapabilitiesReport, AppError> {
    Ok(get_capabilities())
}

/// Enumerate available audio output devices and their supported sample rate ranges.
#[tauri::command]
pub async fn get_lossless_devices() -> Result<Vec<AudioDeviceFormatInfo>, AppError> {
    Ok(list_output_devices())
}

/// Probe a local audio file and return its metadata and format details.
#[tauri::command]
pub async fn probe_local_audio_file(path: String) -> Result<LocalLosslessTrack, AppError> {
    probe_file(Path::new(&path))
}

/// Play a local lossless audio file (FLAC, WAV, ALAC).
#[tauri::command]
pub async fn play_local_file(app: AppHandle, path: String) -> Result<AudioFormatInfo, AppError> {
    // Silence Spotify and YouTube backends if playing
    {
        let playback = app.state::<AppState>().playback.clone();
        let guard = playback.lock().await;
        if let Some(inner) = guard.as_ref() {
            let _ = inner.pause();
        }
    }
    {
        let yt = app.state::<AppState>().yt.clone();
        let guard = yt.lock().await;
        if let Some(player) = guard.as_ref() {
            let _ = player.pause();
        }
    }

    ensure_lossless(&app).await?;

    let lossless = app.state::<AppState>().lossless.clone();
    let guard = lossless.lock().await;
    let player = guard
        .as_ref()
        .ok_or_else(|| AppError::Playback("Lossless player engine unavailable".into()))?;

    let format_info = player.play_file(Path::new(&path), 0)?;

    // Update global state format
    let current_format = app.state::<AppState>().current_format.clone();
    *current_format.lock().await = Some(format_info.clone());

    Ok(format_info)
}

/// Open a native file picker dialog for lossless audio files.
#[tauri::command]
pub async fn pick_local_audio_file() -> Result<Option<String>, AppError> {
    let file = rfd::AsyncFileDialog::new()
        .set_title("Select Lossless Audio Track")
        .add_filter("Lossless Audio (*.flac, *.wav, *.alac)", &["flac", "wav", "wave", "alac"])
        .pick_file()
        .await;

    Ok(file.map(|f| f.path().to_string_lossy().to_string()))
}

/// Open a native folder picker dialog for the local lossless music folder.
#[tauri::command]
pub async fn pick_local_audio_folder() -> Result<Option<String>, AppError> {
    let folder = rfd::AsyncFileDialog::new()
        .set_title("Select Lossless Music Library Folder")
        .pick_folder()
        .await;

    Ok(folder.map(|f| f.path().to_string_lossy().to_string()))
}

/// Scan a lossless audio folder and index tracks into SQLite.
#[tauri::command]
pub async fn scan_lossless_folder(
    app: AppHandle,
    folder: Option<String>,
) -> Result<Vec<LocalLosslessTrack>, AppError> {
    let pool = app.state::<AppState>().db.clone();
    let target_folder = match folder {
        Some(f) if !f.trim().is_empty() => {
            // Save folder preference
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            let _ = sqlx::query(
                "INSERT INTO settings (key, value, updated_at) VALUES ('lossless_folder', ?, ?)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            )
            .bind(&f)
            .bind(now)
            .execute(&pool)
            .await;
            f
        }
        _ => {
            let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'lossless_folder'")
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten();
            row.map(|(v,)| v).unwrap_or_default()
        }
    };

    if target_folder.trim().is_empty() {
        return Ok(Vec::new());
    }

    scan_directory(&pool, Path::new(&target_folder)).await
}

/// Retrieve all scanned local lossless library tracks.
#[tauri::command]
pub async fn get_local_lossless_tracks(app: AppHandle) -> Result<Vec<LocalLosslessTrack>, AppError> {
    let pool = app.state::<AppState>().db.clone();
    get_all_local_tracks(&pool).await
}

/// Set preference: whether to prefer local lossless file over lossy streaming for matched tracks.
#[tauri::command]
pub async fn set_lossless_prefer_local(app: AppHandle, enabled: bool) -> Result<(), AppError> {
    let pool = app.state::<AppState>().db.clone();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES ('lossless_prefer_local', ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(if enabled { "1" } else { "0" })
    .bind(now)
    .execute(&pool)
    .await?;
    Ok(())
}

/// Get preference: whether to prefer local lossless copy when available.
#[tauri::command]
pub async fn get_lossless_prefer_local(app: AppHandle) -> Result<bool, AppError> {
    let pool = app.state::<AppState>().db.clone();
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'lossless_prefer_local'")
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();
    Ok(row.map(|(v,)| v == "1").unwrap_or(true))
}
