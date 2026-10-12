export interface AudioFormatInfo {
  codec: string;
  bit_depth: number | null;
  sample_rate: number;
  channels: number;
  bitrate_kbps: number | null;
  is_lossless: boolean;
  is_hi_res: boolean;
  source_type: "local_lossless" | "spotify" | "youtube" | string;
  output_sample_rate: number;
  output_bit_depth: number | null;
  is_bit_perfect: boolean;
  file_path: string | null;
}

export interface LocalLosslessTrack {
  id: string;
  file_path: string;
  title: string;
  artist: string;
  album: string;
  duration_ms: number;
  format: AudioFormatInfo;
}

export interface AudioDeviceFormatInfo {
  name: string;
  is_default: boolean;
  default_sample_rate: number;
  supported_sample_rates: number[];
  default_channels: number;
  supports_exclusive: boolean;
}

export interface LosslessCapabilitiesReport {
  spotify_lossless_supported: boolean;
  spotify_limitation_reason: string;
  supported_local_formats: string[];
  max_supported_sample_rate: number;
  supported_bit_depths: number[];
  supports_bit_perfect: boolean;
  active_device: AudioDeviceFormatInfo | null;
}
