use std::path::{Path, PathBuf};
use sqlx::SqlitePool;

use crate::errors::AppError;
use super::decoder::probe_file;
use super::types::{AudioFormatInfo, LocalLosslessTrack};

const LOSSLESS_EXTENSIONS: &[&str] = &["flac", "wav", "wave", "alac"];

fn collect_files_recursive(dir: &Path, paths: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(&path, paths);
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if LOSSLESS_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                        paths.push(path);
                    }
                }
            }
        }
    }
}

/// Normalize title or artist string for fuzzy comparison.
/// Strips punctuation, casing, and common tags like "(Remastered)", "- 2011 Remaster", etc.
pub fn normalize_string(s: &str) -> String {
    let lower = s.to_lowercase();
    let base = if let Some((left, right)) = lower.split_once(" - ") {
        if right.contains("remaster")
            || right.contains("live")
            || right.contains("deluxe")
            || right.contains("mono")
            || right.contains("stereo")
            || right.contains("edition")
            || right.contains("version")
        {
            left
        } else {
            &lower
        }
    } else {
        &lower
    };

    let mut clean = String::new();
    let mut in_paren = false;
    let mut in_bracket = false;

    for c in base.chars() {
        if c == '(' {
            in_paren = true;
            continue;
        } else if c == ')' {
            in_paren = false;
            continue;
        } else if c == '[' {
            in_bracket = true;
            continue;
        } else if c == ']' {
            in_bracket = false;
            continue;
        }

        if in_paren || in_bracket {
            continue;
        }

        if c.is_alphanumeric() || c.is_whitespace() {
            clean.push(c);
        }
    }

    clean.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Recursively scan a folder for lossless audio files and upsert them into the database.
pub async fn scan_directory(pool: &SqlitePool, dir: &Path) -> Result<Vec<LocalLosslessTrack>, AppError> {
    if !dir.exists() || !dir.is_dir() {
        return Err(AppError::InvalidInput(format!(
            "Directory does not exist or is not a folder: {}",
            dir.display()
        )));
    }

    let mut paths: Vec<PathBuf> = Vec::new();
    collect_files_recursive(dir, &mut paths);

    let mut tracks = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    for path in paths {
        match probe_file(&path) {
            Ok(track) => {
                let bit_depth_i64 = track.format.bit_depth.map(|bd| bd as i64);
                let bitrate_i64 = track.format.bitrate_kbps.map(|br| br as i64);
                let is_lossless_int = if track.format.is_lossless { 1 } else { 0 };
                let is_hi_res_int = if track.format.is_hi_res { 1 } else { 0 };

                let query_res = sqlx::query(
                    r#"
                    INSERT INTO local_tracks (
                        id, file_path, title, artist, album, duration_ms,
                        codec, sample_rate, bit_depth, channels, bitrate_kbps,
                        is_lossless, is_hi_res, scanned_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(file_path) DO UPDATE SET
                        title = excluded.title,
                        artist = excluded.artist,
                        album = excluded.album,
                        duration_ms = excluded.duration_ms,
                        codec = excluded.codec,
                        sample_rate = excluded.sample_rate,
                        bit_depth = excluded.bit_depth,
                        channels = excluded.channels,
                        bitrate_kbps = excluded.bitrate_kbps,
                        is_lossless = excluded.is_lossless,
                        is_hi_res = excluded.is_hi_res,
                        scanned_at = excluded.scanned_at
                    "#,
                )
                .bind(&track.id)
                .bind(&track.file_path)
                .bind(&track.title)
                .bind(&track.artist)
                .bind(&track.album)
                .bind(track.duration_ms as i64)
                .bind(&track.format.codec)
                .bind(track.format.sample_rate as i64)
                .bind(bit_depth_i64)
                .bind(track.format.channels as i64)
                .bind(bitrate_i64)
                .bind(is_lossless_int)
                .bind(is_hi_res_int)
                .bind(now)
                .execute(pool)
                .await;

                if let Err(e) = query_res {
                    log::warn!("Failed to store local track {}: {e}", path.display());
                } else {
                    tracks.push(track);
                }
            }
            Err(e) => {
                log::warn!("Skipping unparseable audio file {}: {e}", path.display());
            }
        }
    }

    Ok(tracks)
}

/// Retrieve all indexed local lossless tracks.
pub async fn get_all_local_tracks(pool: &SqlitePool) -> Result<Vec<LocalLosslessTrack>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String,
        file_path: String,
        title: String,
        artist: String,
        album: String,
        duration_ms: i64,
        codec: String,
        sample_rate: i64,
        bit_depth: Option<i64>,
        channels: i64,
        bitrate_kbps: Option<i64>,
        is_lossless: i64,
        is_hi_res: i64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT id, file_path, title, artist, album, duration_ms,
               codec, sample_rate, bit_depth, channels, bitrate_kbps,
               is_lossless, is_hi_res
        FROM local_tracks
        ORDER BY artist COLLATE NOCASE ASC, album COLLATE NOCASE ASC, title COLLATE NOCASE ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let tracks = rows
        .into_iter()
        .map(|r| {
            let sample_rate = r.sample_rate as u32;
            let bit_depth = r.bit_depth.map(|bd| bd as u32);
            let format = AudioFormatInfo {
                codec: r.codec,
                bit_depth,
                sample_rate,
                channels: r.channels as u16,
                bitrate_kbps: r.bitrate_kbps.map(|br| br as u32),
                is_lossless: r.is_lossless != 0,
                is_hi_res: r.is_hi_res != 0,
                source_type: "local_lossless".to_string(),
                output_sample_rate: sample_rate,
                output_bit_depth: Some(24),
                is_bit_perfect: true,
                file_path: Some(r.file_path.clone()),
            };

            LocalLosslessTrack {
                id: r.id,
                file_path: r.file_path,
                title: r.title,
                artist: r.artist,
                album: r.album,
                duration_ms: r.duration_ms.max(0) as u64,
                format,
            }
        })
        .collect();

    Ok(tracks)
}

/// Retrieve a single local track by ID or file path.
pub async fn get_local_track(pool: &SqlitePool, id_or_path: &str) -> Result<Option<LocalLosslessTrack>, AppError> {
    let all = get_all_local_tracks(pool).await?;
    Ok(all.into_iter().find(|t| t.id == id_or_path || t.file_path == id_or_path))
}

/// Check if a local lossless file exists that matches a Spotify track (by title and artist).
pub async fn find_lossless_match(
    pool: &SqlitePool,
    spotify_title: &str,
    spotify_artist: &str,
) -> Result<Option<LocalLosslessTrack>, AppError> {
    let norm_title = normalize_string(spotify_title);
    let norm_artist = normalize_string(spotify_artist);

    if norm_title.is_empty() {
        return Ok(None);
    }

    let tracks = get_all_local_tracks(pool).await?;
    for track in tracks {
        let local_title = normalize_string(&track.title);
        let local_artist = normalize_string(&track.artist);

        let title_matches = local_title == norm_title
            || (local_title.len() > 3 && norm_title.contains(&local_title))
            || (norm_title.len() > 3 && local_title.contains(&norm_title));

        let artist_matches = norm_artist.is_empty()
            || local_artist == norm_artist
            || local_artist.contains(&norm_artist)
            || norm_artist.contains(&local_artist);

        if title_matches && artist_matches {
            return Ok(Some(track));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_string_strips_remaster_and_parentheses() {
        assert_eq!(normalize_string("Bohemian Rhapsody (2011 Remaster)"), "bohemian rhapsody");
        assert_eq!(normalize_string("Hotel California - 2013 Remaster"), "hotel california");
        assert_eq!(normalize_string("Starboy [feat. Daft Punk]"), "starboy");
        assert_eq!(normalize_string("Stayin' Alive - Remastered"), "stayin alive");
    }

    #[test]
    fn test_normalize_string_punctuation() {
        assert_eq!(normalize_string("AC/DC"), "acdc");
        assert_eq!(normalize_string("Don't Stop Me Now"), "dont stop me now");
        assert_eq!(normalize_string("Rock & Roll"), "rock roll");
    }
}
