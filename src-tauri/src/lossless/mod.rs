pub mod decoder;
pub mod devices;
pub mod library;
pub mod player;
pub mod types;

pub use decoder::{identify_codec, probe_file, LosslessDecoder};
pub use devices::{get_default_device_info, list_output_devices};
pub use library::{find_lossless_match, get_all_local_tracks, get_local_track, scan_directory};
pub use player::LosslessPlayer;
pub use types::{AudioDeviceFormatInfo, AudioFormatInfo, LocalLosslessTrack, LosslessCapabilitiesReport};

/// Generate an accurate, transparent capabilities report detailing audio support
/// and documenting Spotify's stream encryption and CDN limits.
pub fn get_capabilities() -> LosslessCapabilitiesReport {
    LosslessCapabilitiesReport {
        spotify_lossless_supported: false,
        spotify_limitation_reason: "Spotify streams audio to third-party clients and librespot exclusively in lossy Ogg Vorbis format at 96 kbps, 160 kbps, and 320 kbps. Spotify's Access Point servers and CDN do not provision audio decryption keys for FLAC or lossless streams to third-party clients or the Web Playback SDK. Musique's local engine delivers genuine bit-perfect lossless playback up to 24-bit / 192 kHz.".to_string(),
        supported_local_formats: vec![
            "FLAC (16-bit, 24-bit, 32-bit)".to_string(),
            "PCM WAV (16-bit, 24-bit, 32-bit, IEEE Float)".to_string(),
            "ALAC (Apple Lossless Audio Codec)".to_string(),
        ],
        max_supported_sample_rate: 192000,
        supported_bit_depths: vec![16, 24, 32],
        supports_bit_perfect: true,
        active_device: get_default_device_info(),
    }
}
