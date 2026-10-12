use std::fs::File;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecType, Decoder, DecoderOptions, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC};
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, Tag, Value};
use symphonia::core::units::Time;
use symphonia::default::{get_codecs, get_probe};

use super::types::{AudioFormatInfo, LocalLosslessTrack};
use crate::errors::AppError;

/// Identify codec string and verify whether it represents a lossless format.
pub fn identify_codec(codec: CodecType, ext: &str) -> (&'static str, bool, Option<u32>) {
    match codec {
        CODEC_TYPE_FLAC => ("FLAC", true, None), // bit depth determined from track parameters
        CODEC_TYPE_ALAC => ("ALAC", true, None),
        c if c == symphonia::core::codecs::CODEC_TYPE_PCM_S16LE
            || c == symphonia::core::codecs::CODEC_TYPE_PCM_S16BE =>
        {
            ("PCM WAV", true, Some(16))
        }
        c if c == symphonia::core::codecs::CODEC_TYPE_PCM_S24LE
            || c == symphonia::core::codecs::CODEC_TYPE_PCM_S24BE =>
        {
            ("PCM WAV", true, Some(24))
        }
        c if c == symphonia::core::codecs::CODEC_TYPE_PCM_S32LE
            || c == symphonia::core::codecs::CODEC_TYPE_PCM_S32BE =>
        {
            ("PCM WAV", true, Some(32))
        }
        c if c == symphonia::core::codecs::CODEC_TYPE_PCM_F32LE
            || c == symphonia::core::codecs::CODEC_TYPE_PCM_F32BE =>
        {
            ("WAV (Float)", true, Some(32))
        }
        _ => {
            let ext_lower = ext.to_ascii_lowercase();
            if ext_lower == "flac" {
                ("FLAC", true, None)
            } else if ext_lower == "wav" || ext_lower == "wave" {
                ("WAV", true, None)
            } else if ext_lower == "alac" {
                ("ALAC", true, None)
            } else {
                ("Audio", false, None)
            }
        }
    }
}

/// Probe a local audio file and extract accurate lossless metadata, tags, and container info.
pub fn probe_file(path: &Path) -> Result<LocalLosslessTrack, AppError> {
    let file = File::open(path)
        .map_err(|e| AppError::InvalidInput(format!("Cannot open audio file {}: {e}", path.display())))?;
    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = symphonia::core::probe::Hint::new();
    if !ext.is_empty() {
        hint.with_extension(ext);
    }

    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let meta_opts = MetadataOptions::default();

    let probed = get_probe()
        .format(&hint, mss, &format_opts, &meta_opts)
        .map_err(|e| AppError::Playback(format!("Audio format probing failed for {}: {e}", path.display())))?;

    let mut format_reader = probed.format;
    let track = format_reader
        .default_track()
        .or_else(|| format_reader.tracks().first())
        .ok_or_else(|| AppError::Playback("No audio tracks found in container".into()))?;

    let params = &track.codec_params;
    let (codec_name, is_lossless, default_bd) = identify_codec(params.codec, ext);

    let sample_rate = params.sample_rate.unwrap_or(44100);
    let channels = params.channels.map(|c| c.count() as u16).unwrap_or(2);
    let bit_depth = params.bits_per_sample.or(default_bd).or(if is_lossless { Some(16) } else { None });

    let mut duration_ms = 0u64;
    if let (Some(n_frames), Some(tb)) = (params.n_frames, params.time_base) {
        let time = tb.calc_time(n_frames);
        duration_ms = (time.seconds * 1000) + (time.frac * 1000.0) as u64;
    }

    // Extract tags from metadata revisions
    let mut title = None;
    let mut artist = None;
    let mut album = None;

    let mut extract_tags = |tags: &[Tag]| {
        for tag in tags {
            let key = tag.std_key.as_ref();
            let val = match &tag.value {
                Value::String(s) => s.clone(),
                Value::Binary(b) => String::from_utf8_lossy(b).to_string(),
                _ => continue,
            };
            match key {
                Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                    if title.is_none() { title = Some(val); }
                }
                Some(symphonia::core::meta::StandardTagKey::Artist) => {
                    if artist.is_none() { artist = Some(val); }
                }
                Some(symphonia::core::meta::StandardTagKey::Album) => {
                    if album.is_none() { album = Some(val); }
                }
                _ => {
                    let k = tag.key.to_ascii_uppercase();
                    if k == "TITLE" && title.is_none() {
                        title = Some(val);
                    } else if (k == "ARTIST" || k == "ALBUMARTIST") && artist.is_none() {
                        artist = Some(val);
                    } else if k == "ALBUM" && album.is_none() {
                        album = Some(val);
                    }
                }
            }
        }
    };

    if let Some(metadata) = format_reader.metadata().current() {
        extract_tags(metadata.tags());
    }

    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown Track");
    let title = title.unwrap_or_else(|| file_stem.to_string());
    let artist = artist.unwrap_or_else(|| "Local Artist".to_string());
    let album = album.unwrap_or_else(|| "Local Music".to_string());

    // Calculate approximate bitrate
    let bitrate_kbps = if duration_ms > 0 && file_size > 0 {
        let secs = duration_ms as f64 / 1000.0;
        Some(((file_size as f64 * 8.0) / secs / 1000.0).round() as u32)
    } else if let (Some(bd), sr) = (bit_depth, sample_rate) {
        Some((sr * bd * channels as u32) / 1000)
    } else {
        None
    };

    let is_hi_res = sample_rate > 48000 || bit_depth.unwrap_or(16) > 16;
    let format_info = AudioFormatInfo {
        codec: codec_name.to_string(),
        bit_depth,
        sample_rate,
        channels,
        bitrate_kbps,
        is_lossless,
        is_hi_res,
        source_type: "local_lossless".to_string(),
        output_sample_rate: sample_rate,
        output_bit_depth: Some(24),
        is_bit_perfect: true,
        file_path: Some(path.to_string_lossy().to_string()),
    };

    let id = format!("local:{}", crate::auth::compute_device_id(&path.to_string_lossy()));

    Ok(LocalLosslessTrack {
        id,
        file_path: path.to_string_lossy().to_string(),
        title,
        artist,
        album,
        duration_ms,
        format: format_info,
    })
}

/// Active lossless stream decoder wrapping Symphonia's FormatReader and Decoder.
pub struct LosslessDecoder {
    format_reader: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    duration_ms: u64,
    current_time_ms: f64,
    time_base: Option<symphonia::core::units::TimeBase>,
}

impl LosslessDecoder {
    /// Open a lossless audio file for real-time decoding.
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let file = File::open(path)
            .map_err(|e| AppError::InvalidInput(format!("Failed to open file {}: {e}", path.display())))?;
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");

        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = symphonia::core::probe::Hint::new();
        if !ext.is_empty() {
            hint.with_extension(ext);
        }

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let meta_opts = MetadataOptions::default();

        let probed = get_probe()
            .format(&hint, mss, &format_opts, &meta_opts)
            .map_err(|e| AppError::Playback(format!("Format probe error for {}: {e}", path.display())))?;

        let format_reader = probed.format;
        let track = format_reader
            .default_track()
            .or_else(|| format_reader.tracks().first())
            .ok_or_else(|| AppError::Playback("No audio tracks in file".into()))?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
        let time_base = track.codec_params.time_base;

        let mut duration_ms = 0u64;
        if let (Some(n_frames), Some(tb)) = (track.codec_params.n_frames, time_base) {
            let time = tb.calc_time(n_frames);
            duration_ms = (time.seconds * 1000) + (time.frac * 1000.0) as u64;
        }

        let decoder_opts = DecoderOptions::default();
        let decoder = get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| AppError::Playback(format!("Failed to initialize audio decoder: {e}")))?;

        Ok(Self {
            format_reader,
            decoder,
            track_id,
            sample_rate,
            channels,
            duration_ms,
            current_time_ms: 0.0,
            time_base,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn current_position_ms(&self) -> u32 {
        self.current_time_ms.max(0.0).round() as u32
    }

    /// Seek to a millisecond position in the audio stream.
    pub fn seek(&mut self, position_ms: u32) -> Result<u32, AppError> {
        let secs = position_ms as f64 / 1000.0;
        let make_seek = || SeekTo::Time {
            time: Time::from(secs),
            track_id: Some(self.track_id),
        };

        match self.format_reader.seek(SeekMode::Accurate, make_seek()) {
            Ok(seeked_to) => {
                self.decoder.reset();
                if let Some(tb) = self.time_base {
                    let time = tb.calc_time(seeked_to.actual_ts);
                    let actual_ms = (time.seconds as f64 * 1000.0) + (time.frac * 1000.0);
                    self.current_time_ms = actual_ms;
                    Ok(actual_ms.round() as u32)
                } else {
                    self.current_time_ms = position_ms as f64;
                    Ok(position_ms)
                }
            }
            Err(e) => {
                log::warn!("Accurate seek failed ({e}); attempting coarse seek");
                self.format_reader
                    .seek(SeekMode::Coarse, make_seek())
                    .map_err(|err| AppError::Playback(format!("Audio seek error: {err}")))?;
                self.decoder.reset();
                self.current_time_ms = position_ms as f64;
                Ok(position_ms)
            }
        }
    }

    /// Decode the next packet into interleaved f32 PCM samples.
    /// Returns None when the end of the stream is reached.
    pub fn next_packet(&mut self) -> Result<Option<Vec<f32>>, AppError> {
        loop {
            let packet = match self.format_reader.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => {
                    log::warn!("Packet reading finished or error: {e}");
                    return Ok(None);
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                    sample_buf.copy_interleaved_ref(decoded);
                    let samples = sample_buf.samples().to_vec();

                    let frames = samples.len() / self.channels as usize;
                    if self.sample_rate > 0 {
                        self.current_time_ms += (frames as f64 * 1000.0) / self.sample_rate as f64;
                    }

                    return Ok(Some(samples));
                }
                Err(symphonia::core::errors::Error::DecodeError(msg)) => {
                    log::warn!("Transient decode error: {msg}");
                    continue;
                }
                Err(e) => {
                    return Err(AppError::Playback(format!("Fatal packet decode error: {e}")));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_wav(
        path: &Path,
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        num_frames: u32,
    ) -> std::io::Result<()> {
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_size = num_frames * block_align as u32;
        let file_size = 36 + data_size;

        let mut file = std::fs::File::create(path)?;
        // RIFF header
        file.write_all(b"RIFF")?;
        file.write_all(&file_size.to_le_bytes())?;
        file.write_all(b"WAVE")?;

        // fmt subchunk
        file.write_all(b"fmt ")?;
        file.write_all(&16u32.to_le_bytes())?; // Subchunk1Size (16 for PCM)
        file.write_all(&1u16.to_le_bytes())?;  // AudioFormat (1 for PCM)
        file.write_all(&channels.to_le_bytes())?;
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits_per_sample.to_le_bytes())?;

        // data subchunk
        file.write_all(b"data")?;
        file.write_all(&data_size.to_le_bytes())?;

        // Write silence / zero PCM samples
        let sample_bytes = vec![0u8; data_size as usize];
        file.write_all(&sample_bytes)?;
        file.flush()?;

        Ok(())
    }

    #[test]
    fn test_probe_16bit_44k_wav() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("musique_test_16bit_44k.wav");

        write_test_wav(&test_file, 44100, 2, 16, 4410).expect("failed to write test wav");

        let probe_res = probe_file(&test_file).expect("probe failed");
        assert!(probe_res.format.is_lossless);
        assert_eq!(probe_res.format.sample_rate, 44100);
        assert_eq!(probe_res.format.channels, 2);
        assert_eq!(probe_res.format.bit_depth, Some(16));
        assert_eq!(probe_res.format.codec, "PCM WAV");

        let _ = std::fs::remove_file(test_file);
    }

    #[test]
    fn test_probe_and_decode_24bit_96k_hires_wav() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("musique_test_24bit_96k.wav");

        // 9600 frames = 0.1 seconds at 96kHz
        write_test_wav(&test_file, 96000, 2, 24, 9600).expect("failed to write test hires wav");

        let probe_res = probe_file(&test_file).expect("hires probe failed");
        assert!(probe_res.format.is_lossless);
        assert_eq!(probe_res.format.sample_rate, 96000);
        assert_eq!(probe_res.format.channels, 2);
        assert_eq!(probe_res.format.bit_depth, Some(24));

        let mut decoder = LosslessDecoder::open(&test_file).expect("failed to open decoder");
        assert_eq!(decoder.sample_rate(), 96000);
        assert_eq!(decoder.channels(), 2);

        let packet = decoder.next_packet().expect("packet decode failed");
        assert!(packet.is_some());
        let samples = packet.unwrap();
        assert!(!samples.is_empty());

        let _ = std::fs::remove_file(test_file);
    }
}
