use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use rodio::Source;
use tauri::{AppHandle, Emitter};

use crate::errors::AppError;
use crate::media_controls::MediaMsg;
use crate::playback::{PlayerMsg, SharedVolume};
use crate::playout::{self, Ring};
use crate::resample::Resampler;
use super::decoder::{probe_file, LosslessDecoder};
use super::types::{AudioFormatInfo, LocalLosslessTrack};

const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Adapter source feeding decoded samples from `LosslessDecoder` into rodio.
struct LosslessSource {
    decoder: LosslessDecoder,
    packet_buf: Vec<f32>,
    packet_idx: usize,
    channels: u16,
    sample_rate: u32,
    duration_ms: u64,
}

impl LosslessSource {
    fn new(decoder: LosslessDecoder) -> Self {
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        let duration_ms = decoder.duration_ms();
        Self {
            decoder,
            packet_buf: Vec::new(),
            packet_idx: 0,
            channels,
            sample_rate,
            duration_ms,
        }
    }
}

impl Iterator for LosslessSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        while self.packet_idx >= self.packet_buf.len() {
            match self.decoder.next_packet() {
                Ok(Some(samples)) => {
                    self.packet_buf = samples;
                    self.packet_idx = 0;
                }
                _ => return None,
            }
        }
        let sample = self.packet_buf[self.packet_idx];
        self.packet_idx += 1;
        Some(sample)
    }
}

impl Source for LosslessSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.channels as rodio::ChannelCount
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate as rodio::SampleRate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_millis(self.duration_ms))
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.packet_buf.clear();
        self.packet_idx = 0;
        self.decoder
            .seek(pos.as_millis() as u32)
            .map_err(|_| rodio::source::SeekError::NotSupported {
                underlying_source: "lossless decoder seek",
            })?;
        Ok(())
    }
}

/// Sinc-resampled source adapter when output device doesn't match track sample rate.
struct ResampledSource {
    inner: LosslessSource,
    resampler: Resampler,
    out_buf: Vec<f32>,
    out_idx: usize,
    output_rate: u32,
    channels: u16,
}

impl ResampledSource {
    fn new(inner: LosslessSource, output_rate: u32) -> Result<Self, AppError> {
        let channels = inner.channels;
        let from_rate = inner.sample_rate;
        let resampler = Resampler::new(from_rate, output_rate, channels as usize)
            .ok_or_else(|| AppError::Playback("Sample rates match or invalid".into()))?;
        Ok(Self {
            inner,
            resampler,
            out_buf: Vec::new(),
            out_idx: 0,
            output_rate,
            channels,
        })
    }
}

impl Iterator for ResampledSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        while self.out_idx >= self.out_buf.len() {
            let mut chunk = Vec::with_capacity(1024 * self.channels as usize);
            for _ in 0..(1024 * self.channels as usize) {
                if let Some(s) = self.inner.next() {
                    chunk.push(s);
                } else {
                    break;
                }
            }
            if chunk.is_empty() {
                return None;
            }
            self.out_buf = self.resampler.process(&chunk);
            self.out_idx = 0;
            if self.out_buf.is_empty() {
                continue;
            }
        }
        let sample = self.out_buf[self.out_idx];
        self.out_idx += 1;
        Some(sample)
    }
}

impl Source for ResampledSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.channels as rodio::ChannelCount
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.output_rate as rodio::SampleRate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        self.out_buf.clear();
        self.out_idx = 0;
        self.inner.try_seek(pos)?;
        if let Some(r) = Resampler::new(self.inner.sample_rate, self.output_rate, self.channels as usize) {
            self.resampler = r;
        }
        Ok(())
    }
}

/// The Lossless Audio Player engine.
/// Provides bit-perfect output matching, gapless prebuffering, and format reporting.
pub struct LosslessPlayer {
    app: AppHandle,
    volume: SharedVolume,
    media_tx: std::sync::mpsc::SyncSender<MediaMsg>,
    current_track: Arc<Mutex<Option<LocalLosslessTrack>>>,
    current_format: Arc<Mutex<Option<AudioFormatInfo>>>,
    is_playing: Arc<AtomicBool>,
    is_ended: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    sink: Arc<Mutex<Option<Arc<rodio::Sink>>>>,
    _stream: Arc<Mutex<Option<rodio::OutputStream>>>,
    ring: Arc<Mutex<Option<Arc<Ring>>>>,
    active_position_ms: Arc<AtomicU64>,
    watch_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl LosslessPlayer {
    pub fn new(
        app: AppHandle,
        volume: SharedVolume,
        media_tx: std::sync::mpsc::SyncSender<MediaMsg>,
    ) -> Self {
        Self {
            app,
            volume,
            media_tx,
            current_track: Arc::new(Mutex::new(None)),
            current_format: Arc::new(Mutex::new(None)),
            is_playing: Arc::new(AtomicBool::new(false)),
            is_ended: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            sink: Arc::new(Mutex::new(None)),
            _stream: Arc::new(Mutex::new(None)),
            ring: Arc::new(Mutex::new(None)),
            active_position_ms: Arc::new(AtomicU64::new(0)),
            watch_task: Mutex::new(None),
        }
    }

    /// Play a local lossless audio file (FLAC, WAV, ALAC).
    pub fn play_file(&self, path: &Path, position_ms: u32) -> Result<AudioFormatInfo, AppError> {
        log::info!("LosslessPlayer: Opening {}", path.display());
        let track = probe_file(path)?;
        let file_rate = track.format.sample_rate;
        let channels = track.format.channels;

        // Bump generation to cancel prior watchers
        let gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Stop prior sink
        {
            let mut sink_guard = self.sink.lock().unwrap();
            if let Some(old_sink) = sink_guard.take() {
                old_sink.stop();
            }
        }

        // Open audio output stream
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AppError::Playback("No default audio output device found".into()))?;

        let supported_buffer = device
            .default_output_config()
            .map(|c| *c.buffer_size())
            .unwrap_or(cpal::SupportedBufferSize::Unknown);


        let opened_stream = (|| -> Result<(rodio::OutputStream, u32, bool), AppError> {
            let buffer_frames = (u64::from(file_rate) * 100 / 1000) as u32;
            let buffer_size = match supported_buffer {
                cpal::SupportedBufferSize::Range { min, max } if min <= max && max > 0 => {
                    cpal::BufferSize::Fixed(buffer_frames.clamp(min.max(1), max))
                }
                _ => cpal::BufferSize::Fixed(buffer_frames),
            };

            let builder = rodio::OutputStreamBuilder::from_device(device.clone())
                .map_err(|e| AppError::Playback(format!("Output device error: {e}")))?
                .with_channels(channels as rodio::ChannelCount)
                .with_sample_rate(file_rate as rodio::SampleRate)
                .with_buffer_size(buffer_size);

            match builder.open_stream() {
                Ok(s) => Ok((s, file_rate, true)),
                Err(err) => {
                    log::warn!(
                        "Could not open device directly at {} Hz ({err}); trying device default rate",
                        file_rate
                    );
                    let def_cfg = device
                        .default_output_config()
                        .map_err(|e| AppError::Playback(format!("Default config error: {e}")))?;
                    let def_rate = def_cfg.sample_rate().0;
                    let def_buffer_frames = (u64::from(def_rate) * 100 / 1000) as u32;
                    let def_buffer_size = match *def_cfg.buffer_size() {
                        cpal::SupportedBufferSize::Range { min, max } if min <= max && max > 0 => {
                            cpal::BufferSize::Fixed(def_buffer_frames.clamp(min.max(1), max))
                        }
                        _ => cpal::BufferSize::Fixed(def_buffer_frames),
                    };

                    let fallback_builder = rodio::OutputStreamBuilder::from_device(device)
                        .map_err(|e| AppError::Playback(format!("Fallback device error: {e}")))?
                        .with_channels(channels as rodio::ChannelCount)
                        .with_sample_rate(def_rate as rodio::SampleRate)
                        .with_buffer_size(def_buffer_size);

                    let stream = fallback_builder
                        .open_stream_or_fallback()
                        .map_err(|e| AppError::Playback(format!("Stream fallback error: {e}")))?;
                    Ok((stream, def_rate, false))
                }
            }
        })();

        let (stream, actual_output_rate, bit_perfect) = opened_stream?;

        // Open Symphonia decoder
        let decoder = LosslessDecoder::open(path)?;
        let lossless_source = LosslessSource::new(decoder);

        // Prebuffer 4 seconds ahead for continuous glitch-free playback
        let (ring_source, ring) = if actual_output_rate == file_rate {
            playout::prebuffer(
                lossless_source,
                Duration::from_millis(position_ms as u64),
                position_ms > 0,
                None,
            )
        } else {
            let resampled = ResampledSource::new(lossless_source, actual_output_rate)?;
            playout::prebuffer(
                resampled,
                Duration::from_millis(position_ms as u64),
                position_ms > 0,
                None,
            )
        };

        let sink_arc = Arc::new(rodio::Sink::connect_new(stream.mixer()));
        let current_vol = if self.volume.is_muted() {
            0.0
        } else {
            self.volume.level() as f32
        };
        sink_arc.set_volume(current_vol);
        sink_arc.append(ring_source);
        sink_arc.play();

        let mut final_format = track.format.clone();
        final_format.output_sample_rate = actual_output_rate;
        final_format.is_bit_perfect = bit_perfect;
        final_format.file_path = Some(path.to_string_lossy().to_string());

        *self.current_track.lock().unwrap() = Some(track.clone());
        *self.current_format.lock().unwrap() = Some(final_format.clone());
        *self.ring.lock().unwrap() = Some(Arc::clone(&ring));
        *self.sink.lock().unwrap() = Some(Arc::clone(&sink_arc));
        *self._stream.lock().unwrap() = Some(stream);
        self.is_playing.store(true, Ordering::Relaxed);
        self.is_ended.store(false, Ordering::Relaxed);
        self.active_position_ms.store(position_ms as u64, Ordering::Relaxed);

        // Start watcher loop
        let mut watch_guard = self.watch_task.lock().unwrap();
        if let Some(old_task) = watch_guard.take() {
            old_task.abort();
        }

        let app_handle = self.app.clone();
        let media_tx = self.media_tx.clone();
        let track_id_clone = track.id.clone();
        let is_playing_flag = Arc::clone(&self.is_playing);
        let is_ended_flag = Arc::clone(&self.is_ended);
        let generation_flag = Arc::clone(&self.generation);
        let pos_flag = Arc::clone(&self.active_position_ms);
        let ring_clone = Arc::clone(&ring);
        let sink_watcher = Arc::clone(&sink_arc);

        *watch_guard = Some(tauri::async_runtime::spawn(async move {
            let mut last_emit = std::time::Instant::now();
            let mut watching_gen: Option<u64> = None;
            loop {
                tokio::time::sleep(POLL_INTERVAL).await;

                if generation_flag.load(Ordering::SeqCst) != gen {
                    break;
                }

                if is_ended_flag.load(Ordering::Relaxed) {
                    break;
                }

                let queued = !sink_watcher.empty();
                if queued {
                    watching_gen = Some(gen);
                } else if watching_gen == Some(gen) && !is_ended_flag.load(Ordering::Relaxed) {
                    is_ended_flag.store(true, Ordering::Relaxed);
                    is_playing_flag.store(false, Ordering::Relaxed);
                    let _ = media_tx.try_send(MediaMsg::Stopped);
                    let _ = app_handle.emit(
                        "player:event",
                        PlayerMsg::EndOfTrack {
                            track_id: Some(track_id_clone.clone()),
                        },
                    );
                    break;
                }

                let current_ms = ring_clone.heard_ms();
                pos_flag.store(current_ms, Ordering::Relaxed);

                if is_playing_flag.load(Ordering::Relaxed) && last_emit.elapsed() >= Duration::from_millis(900) {
                    last_emit = std::time::Instant::now();
                    let _ = app_handle.emit(
                        "player:event",
                        PlayerMsg::PositionChanged {
                            track_id: Some(track_id_clone.clone()),
                            position_ms: current_ms as u32,
                        },
                    );
                }
            }
        }));

        // Emit format changed and playing event
        let _ = self.app.emit("audio:format-changed", &final_format);
        let _ = self.app.emit(
            "player:event",
            PlayerMsg::Playing {
                track_id: Some(track.id.clone()),
                position_ms,
            },
        );

        let _ = self.media_tx.try_send(MediaMsg::NowPlaying {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            cover_url: None,
            track_url: None,
            duration_ms: track.duration_ms,
            position_ms: position_ms as u64,
        });
        let _ = self.media_tx.try_send(MediaMsg::Playing {
            position_ms: position_ms as u64,
        });

        Ok(final_format)
    }

    pub fn pause(&self) -> Result<(), AppError> {
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.pause();
        }
        if let Some(ring) = self.ring.lock().unwrap().as_ref() {
            ring.set_held(true);
        }
        self.is_playing.store(false, Ordering::Relaxed);

        let track_id = self.current_track.lock().unwrap().as_ref().map(|t| t.id.clone());
        let pos = self.active_position_ms.load(Ordering::Relaxed) as u32;

        let _ = self.app.emit(
            "player:event",
            PlayerMsg::Paused {
                track_id,
                position_ms: pos,
            },
        );
        let _ = self.media_tx.try_send(MediaMsg::Paused {
            position_ms: pos as u64,
        });
        Ok(())
    }

    pub fn resume(&self) -> Result<(), AppError> {
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            sink.play();
        }
        if let Some(ring) = self.ring.lock().unwrap().as_ref() {
            ring.set_held(false);
        }
        self.is_playing.store(true, Ordering::Relaxed);

        let track_id = self.current_track.lock().unwrap().as_ref().map(|t| t.id.clone());
        let pos = self.active_position_ms.load(Ordering::Relaxed) as u32;

        let _ = self.app.emit(
            "player:event",
            PlayerMsg::Playing {
                track_id: track_id.clone(),
                position_ms: pos,
            },
        );

        let _ = self.media_tx.try_send(MediaMsg::Playing {
            position_ms: pos as u64,
        });
        Ok(())
    }

    pub fn seek(&self, position_ms: u32) -> Result<(), AppError> {
        if let Some(ring) = self.ring.lock().unwrap().as_ref() {
            ring.request_seek(Duration::from_millis(position_ms as u64));
            self.active_position_ms.store(position_ms as u64, Ordering::Relaxed);
            let track_id = self.current_track.lock().unwrap().as_ref().map(|t| t.id.clone());
            let _ = self.app.emit(
                "player:event",
                PlayerMsg::PositionChanged {
                    track_id,
                    position_ms,
                },
            );
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), AppError> {
        let mut sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.take() {
            sink.stop();
        }
        self.is_playing.store(false, Ordering::Relaxed);
        let track_id = self.current_track.lock().unwrap().as_ref().map(|t| t.id.clone());
        let _ = self.app.emit("player:event", PlayerMsg::Stopped { track_id });
        let _ = self.media_tx.try_send(MediaMsg::Stopped);
        Ok(())
    }

    pub fn set_level(&self, level: f64) {
        self.volume.set_level(level);
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            let vol = if self.volume.is_muted() { 0.0 } else { level as f32 };
            sink.set_volume(vol);
        }
    }

    pub fn set_muted(&self, muted: bool) {
        self.volume.set_muted(muted);
        let sink_guard = self.sink.lock().unwrap();
        if let Some(sink) = sink_guard.as_ref() {
            let vol = if muted { 0.0 } else { self.volume.level() as f32 };
            sink.set_volume(vol);
        }
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn is_ended(&self) -> bool {
        self.is_ended.load(Ordering::Relaxed)
    }

    pub fn position_ms(&self) -> u32 {
        if let Some(ring) = self.ring.lock().unwrap().as_ref() {
            ring.heard_ms() as u32
        } else {
            self.active_position_ms.load(Ordering::Relaxed) as u32
        }
    }

    pub fn current_track(&self) -> Option<LocalLosslessTrack> {
        self.current_track.lock().unwrap().clone()
    }

    pub fn current_format(&self) -> Option<AudioFormatInfo> {
        self.current_format.lock().unwrap().clone()
    }
}
