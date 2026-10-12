-- Local lossless audio catalog and playback settings
CREATE TABLE IF NOT EXISTS local_tracks (
    id                 TEXT    PRIMARY KEY,
    file_path          TEXT    NOT NULL UNIQUE,
    title              TEXT    NOT NULL,
    artist             TEXT    NOT NULL,
    album              TEXT    NOT NULL,
    duration_ms        INTEGER NOT NULL,
    codec              TEXT    NOT NULL,
    sample_rate        INTEGER NOT NULL,
    bit_depth          INTEGER,
    channels           INTEGER NOT NULL,
    bitrate_kbps       INTEGER,
    is_lossless        INTEGER NOT NULL DEFAULT 1,
    is_hi_res          INTEGER NOT NULL DEFAULT 0,
    spotify_match_id   TEXT,
    scanned_at         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_local_tracks_artist ON local_tracks (artist);
CREATE INDEX IF NOT EXISTS idx_local_tracks_title ON local_tracks (title);
CREATE INDEX IF NOT EXISTS idx_local_tracks_spotify_match ON local_tracks (spotify_match_id);

INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES
    ('lossless_enabled', '1', unixepoch('now') * 1000),
    ('lossless_prefer_local', '1', unixepoch('now') * 1000),
    ('lossless_folder', '', unixepoch('now') * 1000),
    ('exclusive_audio', '0', unixepoch('now') * 1000);
