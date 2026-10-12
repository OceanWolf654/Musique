use serde::{Deserialize, Serialize};

/// Detailed audio format, quality, and signal path information.
/// Accurately reports codec, bit depth, sample rate, channels, and bitrate,
/// distinguishing between genuine lossless sources and lossy streams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioFormatInfo {
    /// Audio codec name (e.g., "FLAC", "ALAC", "WAV", "Ogg Vorbis", "AAC")
    pub codec: String,
    /// Bit depth in bits (e.g., 16, 24, 32). None for lossy codecs where bit depth is not applicable.
    pub bit_depth: Option<u32>,
    /// Sampling rate in Hz (e.g., 44100, 48000, 96000, 192000)
    pub sample_rate: u32,
    /// Channel count (e.g., 2 for Stereo, 1 for Mono, 6 for 5.1)
    pub channels: u16,
    /// Approximate or exact bitrate in kbps (e.g., 1411 for CD 16/44.1, 320 for Spotify Very High)
    pub bitrate_kbps: Option<u32>,
    /// Whether the audio source is genuinely lossless (FLAC, ALAC, WAV PCM)
    pub is_lossless: bool,
    /// Whether this is High-Resolution Audio (> 48 kHz sample rate or > 16-bit depth)
    pub is_hi_res: bool,
    /// Playback source identifier: "local_lossless", "spotify", "youtube"
    pub source_type: String,
    /// Active hardware output sample rate in Hz
    pub output_sample_rate: u32,
    /// Hardware output bit depth if known (e.g. 24 or 32)
    pub output_bit_depth: Option<u32>,
    /// Whether playback is bit-perfect (no resampling or format conversion applied)
    pub is_bit_perfect: bool,
    /// File path if playing from a local file
    pub file_path: Option<String>,
}

impl AudioFormatInfo {
    /// Create format info for Spotify lossy playback.
    /// Spotify streams via librespot in Ogg Vorbis at 96, 160, or 320 kbps (44.1 kHz, 16-bit container).
    pub fn spotify(bitrate_kbps: u32, output_sample_rate: u32) -> Self {
        Self {
            codec: "Ogg Vorbis".to_string(),
            bit_depth: None, // Vorbis is lossy (frequency-domain MDCT), no fixed bit depth
            sample_rate: 44100,
            channels: 2,
            bitrate_kbps: Some(bitrate_kbps),
            is_lossless: false,
            is_hi_res: false,
            source_type: "spotify".to_string(),
            output_sample_rate,
            output_bit_depth: Some(24),
            is_bit_perfect: false,
            file_path: None,
        }
    }

    /// Create format info for YouTube lossy playback.
    pub fn youtube(output_sample_rate: u32) -> Self {
        Self {
            codec: "AAC / Opus".to_string(),
            bit_depth: None,
            sample_rate: 48000,
            channels: 2,
            bitrate_kbps: Some(128),
            is_lossless: false,
            is_hi_res: false,
            source_type: "youtube".to_string(),
            output_sample_rate,
            output_bit_depth: Some(24),
            is_bit_perfect: false,
            file_path: None,
        }
    }

    /// Human-readable quality description badge string.
    pub fn display_badge(&self) -> String {
        if self.is_hi_res {
            "Hi-Res Lossless".to_string()
        } else if self.is_lossless {
            "Lossless".to_string()
        } else if let Some(br) = self.bitrate_kbps {
            format!("{br} kbps")
        } else {
            "Standard".to_string()
        }
    }

    /// Technical format summary (e.g. "FLAC 24-bit / 96 kHz · 2822 kbps").
    pub fn technical_summary(&self) -> String {
        let mut parts = Vec::new();
        parts.push(self.codec.clone());
        if let Some(bd) = self.bit_depth {
            parts.push(format!("{bd}-bit"));
        }
        let sr_khz = self.sample_rate as f64 / 1000.0;
        if (sr_khz.fract() - 0.0).abs() < 0.01 {
            parts.push(format!("{:.0} kHz", sr_khz));
        } else {
            parts.push(format!("{:.1} kHz", sr_khz));
        }
        if let Some(br) = self.bitrate_kbps {
            parts.push(format!("{br} kbps"));
        }
        parts.join(" · ")
    }
}

/// Metadata and format description of a local lossless music track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLosslessTrack {
    pub id: String,
    pub file_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub format: AudioFormatInfo,
}

/// Information about an audio output device and its format support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceFormatInfo {
    pub name: String,
    pub is_default: bool,
    pub default_sample_rate: u32,
    pub supported_sample_rates: Vec<u32>,
    pub default_channels: u16,
    pub supports_exclusive: bool,
}

/// Comprehensive report on client audio capabilities and streaming limitations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LosslessCapabilitiesReport {
    /// True if Spotify provides lossless streams via librespot (currently false).
    pub spotify_lossless_supported: bool,
    /// Detailed factual explanation of Spotify API and CDN restrictions.
    pub spotify_limitation_reason: String,
    /// Lossless formats fully supported by the local lossless engine.
    pub supported_local_formats: Vec<String>,
    /// Maximum sample rate supported by the decoder (up to 192,000 Hz).
    pub max_supported_sample_rate: u32,
    /// Bit-depths supported (16-bit, 24-bit, 32-bit PCM/Float).
    pub supported_bit_depths: Vec<u32>,
    /// Whether bit-perfect output mode is available.
    pub supports_bit_perfect: bool,
    /// Active output device details.
    pub active_device: Option<AudioDeviceFormatInfo>,
}
