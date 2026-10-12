import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getCurrentAudioFormat,
  getLosslessCapabilities,
  getLosslessDevices,
  getLosslessPreferLocal,
  setLosslessPreferLocal,
  probeLocalAudioFile,
  playLocalFile,
} from "./lossless";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const backend = vi.mocked(invoke);

beforeEach(() => {
  backend.mockReset();
});

it("queries current audio format via IPC", async () => {
  const fakeFormat = {
    codec: "FLAC",
    container: "flac",
    bit_depth: 24,
    sample_rate: 96000,
    channels: 2,
    bitrate_kbps: 2822,
    is_lossless: true,
    is_bit_perfect: true,
    output_sample_rate: 96000,
    file_path: "/music/track.flac",
  };
  backend.mockResolvedValueOnce(fakeFormat);

  const res = await getCurrentAudioFormat();
  expect(backend).toHaveBeenCalledWith("get_current_audio_format");
  expect(res).toEqual(fakeFormat);
  expect(res?.is_lossless).toBe(true);
  expect(res?.bit_depth).toBe(24);
  expect(res?.sample_rate).toBe(96000);
});

it("retrieves factual lossless capabilities report", async () => {
  const fakeCaps = {
    spotify_lossless_supported: false,
    spotify_limitation_reason: "Spotify streams exclusively in lossy Ogg Vorbis",
    supported_local_formats: ["FLAC (16-bit, 24-bit, 32-bit)", "PCM WAV", "ALAC"],
    max_supported_sample_rate: 192000,
    supported_bit_depths: [16, 24, 32],
    supports_bit_perfect: true,
    active_device: {
      name: "DAC Out",
      default_sample_rate: 96000,
      supported_sample_rates: [44100, 48000, 96000, 192000],
      is_default: true,
    },
  };
  backend.mockResolvedValueOnce(fakeCaps);

  const caps = await getLosslessCapabilities();
  expect(backend).toHaveBeenCalledWith("get_lossless_capabilities");
  expect(caps.spotify_lossless_supported).toBe(false);
  expect(caps.max_supported_sample_rate).toBe(192000);
  expect(caps.supported_local_formats).toContain("PCM WAV");
});

it("invokes prefer local toggle and direct play commands", async () => {
  backend.mockResolvedValueOnce(true);
  await setLosslessPreferLocal(true);
  expect(backend).toHaveBeenCalledWith("set_lossless_prefer_local", { enabled: true });

  backend.mockResolvedValueOnce(false);
  const pref = await getLosslessPreferLocal();
  expect(backend).toHaveBeenCalledWith("get_lossless_prefer_local");
  expect(pref).toBe(false);

  backend.mockResolvedValueOnce(undefined);
  await playLocalFile("C:\\Music\\Album\\01.flac");
  expect(backend).toHaveBeenCalledWith("play_local_file", { path: "C:\\Music\\Album\\01.flac" });

  backend.mockResolvedValueOnce([]);
  const devices = await getLosslessDevices();
  expect(backend).toHaveBeenCalledWith("get_lossless_devices");
  expect(devices).toEqual([]);

  backend.mockResolvedValueOnce(null);
  const probed = await probeLocalAudioFile("C:\\Music\\test.wav");
  expect(backend).toHaveBeenCalledWith("probe_local_audio_file", { path: "C:\\Music\\test.wav" });
  expect(probed).toBeNull();
});
