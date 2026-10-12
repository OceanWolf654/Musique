import { invoke } from "@tauri-apps/api/core";
import type {
  AudioDeviceFormatInfo,
  AudioFormatInfo,
  LocalLosslessTrack,
  LosslessCapabilitiesReport,
} from "../types/lossless";

/** Retrieve currently active audio format, bit depth, and signal path info */
export const getCurrentAudioFormat = (): Promise<AudioFormatInfo | null> =>
  invoke("get_current_audio_format");

/** Get comprehensive capabilities report documenting Spotify vs Local audio capabilities */
export const getLosslessCapabilities = (): Promise<LosslessCapabilitiesReport> =>
  invoke("get_lossless_capabilities");

/** Enumerate available audio output devices and their sample rate support */
export const getLosslessDevices = (): Promise<AudioDeviceFormatInfo[]> =>
  invoke("get_lossless_devices");

/** Probe a local audio file and return its lossless metadata */
export const probeLocalAudioFile = (path: string): Promise<LocalLosslessTrack> =>
  invoke("probe_local_audio_file", { path });

/** Play a local audio file (FLAC, WAV, ALAC) through the lossless bit-perfect engine */
export const playLocalFile = (path: string): Promise<AudioFormatInfo> =>
  invoke("play_local_file", { path });

/** Open a native file picker dialog to select a lossless audio track */
export const pickLocalAudioFile = (): Promise<string | null> =>
  invoke("pick_local_audio_file");

/** Open a native folder picker dialog to select the local music directory */
export const pickLocalAudioFolder = (): Promise<string | null> =>
  invoke("pick_local_audio_folder");

/** Scan the lossless audio folder and index tracks into the local catalog */
export const scanLosslessFolder = (folder?: string): Promise<LocalLosslessTrack[]> =>
  invoke("scan_lossless_folder", { folder });

/** Get all indexed local lossless tracks */
export const getLocalLosslessTracks = (): Promise<LocalLosslessTrack[]> =>
  invoke("get_local_lossless_tracks");

/** Toggle whether to auto-upgrade Spotify playback to local lossless copies when available */
export const setLosslessPreferLocal = (enabled: boolean): Promise<void> =>
  invoke("set_lossless_prefer_local", { enabled });

/** Check whether auto-upgrade to local lossless copy is enabled */
export const getLosslessPreferLocal = (): Promise<boolean> =>
  invoke("get_lossless_prefer_local");
