import { useState, useRef, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { usePlayerStore } from "../../store/player.store";
import { pickLocalAudioFile, playLocalFile } from "../../api/lossless";
import { toast } from "../../store/toast.store";
import { EASE_OUT } from "../../lib/motion";

export function LosslessBadge({
  compact = false,
  onOpenSettings,
}: {
  compact?: boolean;
  onOpenSettings?: () => void;
}) {
  const audioFormat = usePlayerStore((s) => s.audioFormat);
  const currentTrack = usePlayerStore((s) => s.currentTrack);
  const [modalOpen, setModalOpen] = useState(false);
  const modalRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!modalOpen) return;
    function handleClickOutside(e: MouseEvent) {
      if (modalRef.current && !modalRef.current.contains(e.target as Node)) {
        setModalOpen(false);
      }
    }
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        setModalOpen(false);
      }
    }
    window.addEventListener("mousedown", handleClickOutside);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleClickOutside);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [modalOpen]);

  if (!currentTrack && !audioFormat) {
    return null;
  }

  const isLossless = audioFormat?.is_lossless ?? false;
  const isHiRes =
    isLossless &&
    ((audioFormat?.bit_depth ?? 16) > 16 || (audioFormat?.sample_rate ?? 44100) > 48000);
  const sampleRateKHz = audioFormat?.sample_rate
    ? (audioFormat.sample_rate / 1000).toFixed(1)
    : "44.1";
  const bitDepth = audioFormat?.bit_depth ?? (isLossless ? 16 : null);

  const label = isHiRes
    ? "HI-RES LOSSLESS"
    : isLossless
    ? "LOSSLESS"
    : audioFormat?.bitrate_kbps
    ? `${audioFormat.bitrate_kbps} KBPS`
    : "320 KBPS";

  const handlePickFile = async () => {
    try {
      const picked = await pickLocalAudioFile();
      if (picked) {
        await playLocalFile(picked);
        setModalOpen(false);
        toast(`Playing local file: ${picked.split(/[\\/]/).pop()}`);
      }
    } catch (err) {
      toast.error(`Could not play local file: ${err}`);
    }
  };

  return (
    <>
      <motion.button
        type="button"
        whileHover={{ scale: 1.05 }}
        whileTap={{ scale: 0.96 }}
        onClick={() => setModalOpen(true)}
        title="Click to view Audio Quality & Signal Path"
        aria-label={`Audio format: ${label}`}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          padding: compact ? "1px 6px" : "2px 8px",
          borderRadius: 6,
          fontSize: compact ? 9 : 10,
          fontWeight: 700,
          letterSpacing: "0.06em",
          lineHeight: 1.4,
          border: isHiRes
            ? "1px solid rgba(245, 158, 11, 0.4)"
            : isLossless
            ? "1px solid rgba(14, 165, 233, 0.4)"
            : "1px solid rgba(255, 255, 255, 0.12)",
          background: isHiRes
            ? "linear-gradient(135deg, rgba(245, 158, 11, 0.22) 0%, rgba(217, 119, 6, 0.12) 100%)"
            : isLossless
            ? "linear-gradient(135deg, rgba(14, 165, 233, 0.22) 0%, rgba(2, 132, 199, 0.12) 100%)"
            : "rgba(255, 255, 255, 0.06)",
          color: isHiRes
            ? "#fbbf24"
            : isLossless
            ? "#38bdf8"
            : "rgba(255, 255, 255, 0.65)",
          cursor: "pointer",
          userSelect: "none",
          boxShadow: isHiRes
            ? "0 0 10px rgba(245, 158, 11, 0.18)"
            : isLossless
            ? "0 0 10px rgba(14, 165, 233, 0.18)"
            : "none",
          transition: "all 0.15s ease",
        }}
      >
        {isHiRes ? (
          <span style={{ fontSize: 9 }}>✨</span>
        ) : isLossless ? (
          <span style={{ fontSize: 9 }}>💎</span>
        ) : (
          <span
            style={{
              width: 5,
              height: 5,
              borderRadius: "50%",
              background: "rgba(255, 255, 255, 0.4)",
            }}
          />
        )}
        <span>{label}</span>
      </motion.button>

      {/* Signal Path Modal */}
      <AnimatePresence>
        {modalOpen && (
          <div
            style={{
              position: "fixed",
              inset: 0,
              zIndex: 9999,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              background: "rgba(0, 0, 0, 0.6)",
              backdropFilter: "blur(8px)",
              WebkitBackdropFilter: "blur(8px)",
              padding: 20,
            }}
          >
            <motion.div
              ref={modalRef}
              initial={{ opacity: 0, scale: 0.95, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: 10 }}
              transition={{ duration: 0.18, ease: EASE_OUT }}
              style={{
                width: "100%",
                maxWidth: 480,
                background: "rgba(20, 20, 26, 0.95)",
                border: "1px solid rgba(255, 255, 255, 0.12)",
                borderRadius: 16,
                padding: "24px",
                boxShadow: "0 24px 60px rgba(0, 0, 0, 0.7)",
                color: "#fff",
                fontFamily: "var(--font-sans, inherit)",
              }}
            >
              {/* Header */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  marginBottom: 18,
                }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <div
                    style={{
                      width: 32,
                      height: 32,
                      borderRadius: 8,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                      background: isHiRes
                        ? "rgba(245, 158, 11, 0.2)"
                        : isLossless
                        ? "rgba(14, 165, 233, 0.2)"
                        : "rgba(255, 255, 255, 0.1)",
                      fontSize: 16,
                    }}
                  >
                    {isHiRes ? "✨" : isLossless ? "💎" : "📻"}
                  </div>
                  <div>
                    <h3 style={{ margin: 0, fontSize: 16, fontWeight: 700 }}>
                      Audio Signal Path
                    </h3>
                    <p
                      style={{
                        margin: 0,
                        fontSize: 12,
                        color: "rgba(255, 255, 255, 0.5)",
                      }}
                    >
                      {isHiRes
                        ? "High-Resolution Lossless Audio Pipeline"
                        : isLossless
                        ? "Bit-Perfect Lossless Stream"
                        : "Spotify Standard Playback Stream"}
                    </p>
                  </div>
                </div>

                <button
                  type="button"
                  onClick={() => setModalOpen(false)}
                  style={{
                    background: "rgba(255, 255, 255, 0.08)",
                    border: "none",
                    borderRadius: 6,
                    color: "rgba(255, 255, 255, 0.6)",
                    cursor: "pointer",
                    padding: "4px 8px",
                    fontSize: 13,
                  }}
                >
                  ✕
                </button>
              </div>

              {/* Signal Path Timeline */}
              <div
                style={{
                  display: "flex",
                  flexDirection: "column",
                  gap: 12,
                  marginBottom: 20,
                  background: "rgba(0, 0, 0, 0.3)",
                  padding: "16px",
                  borderRadius: 12,
                  border: "1px solid rgba(255, 255, 255, 0.06)",
                }}
              >
                {/* Node 1: Source */}
                <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
                  <div
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: "50%",
                      background: isLossless ? "#38bdf8" : "rgba(255,255,255,0.4)",
                      marginTop: 4,
                      boxShadow: isLossless ? "0 0 8px #38bdf8" : "none",
                    }}
                  />
                  <div style={{ flex: 1 }}>
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <span style={{ fontSize: 13, fontWeight: 600 }}>Source Format</span>
                      <span
                        style={{
                          fontSize: 12,
                          fontWeight: 700,
                          color: isHiRes ? "#fbbf24" : isLossless ? "#38bdf8" : "inherit",
                        }}
                      >
                        {audioFormat?.codec ?? (isLossless ? "FLAC" : "Ogg Vorbis")}
                      </span>
                    </div>
                    <div
                      style={{
                        fontSize: 12,
                        color: "rgba(255, 255, 255, 0.5)",
                        marginTop: 2,
                      }}
                    >
                      {bitDepth ? `${bitDepth}-bit / ` : ""}
                      {sampleRateKHz} kHz · {audioFormat?.channels ?? 2} Channels
                      {audioFormat?.bitrate_kbps ? ` · ${audioFormat.bitrate_kbps} kbps` : ""}
                    </div>
                  </div>
                </div>

                {/* Node 2: Engine Processing */}
                <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
                  <div
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: "50%",
                      background: isLossless ? "#34d399" : "rgba(255,255,255,0.4)",
                      marginTop: 4,
                      boxShadow: isLossless ? "0 0 8px #34d399" : "none",
                    }}
                  />
                  <div style={{ flex: 1 }}>
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <span style={{ fontSize: 13, fontWeight: 600 }}>Audio Engine</span>
                      <span
                        style={{
                          fontSize: 11,
                          fontWeight: 600,
                          padding: "2px 6px",
                          borderRadius: 4,
                          background: audioFormat?.is_bit_perfect
                            ? "rgba(52, 211, 153, 0.2)"
                            : "rgba(255, 255, 255, 0.1)",
                          color: audioFormat?.is_bit_perfect ? "#34d399" : "inherit",
                        }}
                      >
                        {audioFormat?.is_bit_perfect
                          ? "Bit-Perfect Passthrough"
                          : isLossless
                          ? "Polyphase Sinc Resampling"
                          : "Native Decoder (librespot)"}
                      </span>
                    </div>
                    <div
                      style={{
                        fontSize: 12,
                        color: "rgba(255, 255, 255, 0.5)",
                        marginTop: 2,
                      }}
                    >
                      {isLossless
                        ? "Prebuffered ring buffer (4.0s) · 0 lossy transcode steps"
                        : "Decoded via Vorbis codec · Standard system stream"}
                    </div>
                  </div>
                </div>

                {/* Node 3: Audio Output */}
                <div style={{ display: "flex", alignItems: "flex-start", gap: 12 }}>
                  <div
                    style={{
                      width: 10,
                      height: 10,
                      borderRadius: "50%",
                      background: "#a78bfa",
                      marginTop: 4,
                      boxShadow: "0 0 8px #a78bfa",
                    }}
                  />
                  <div style={{ flex: 1 }}>
                    <div
                      style={{
                        display: "flex",
                        justifyContent: "space-between",
                        alignItems: "center",
                      }}
                    >
                      <span style={{ fontSize: 13, fontWeight: 600 }}>Audio Output</span>
                      <span style={{ fontSize: 12, color: "#a78bfa", fontWeight: 600 }}>
                        {audioFormat?.output_sample_rate
                          ? `${(audioFormat.output_sample_rate / 1000).toFixed(1)} kHz`
                          : `${sampleRateKHz} kHz`}
                      </span>
                    </div>
                    <div
                      style={{
                        fontSize: 12,
                        color: "rgba(255, 255, 255, 0.5)",
                        marginTop: 2,
                      }}
                    >
                      Default WASAPI Output Device
                    </div>
                  </div>
                </div>
              </div>

              {/* Informational Callout */}
              <div
                style={{
                  background: "rgba(255, 255, 255, 0.04)",
                  border: "1px solid rgba(255, 255, 255, 0.08)",
                  borderRadius: 10,
                  padding: "12px 14px",
                  fontSize: 12,
                  lineHeight: 1.5,
                  color: "rgba(255, 255, 255, 0.7)",
                  marginBottom: 18,
                }}
              >
                <strong style={{ color: "#fff", display: "block", marginBottom: 4 }}>
                  ℹ️ Spotify & Lossless Audio Policy
                </strong>
                Spotify delivers streams to desktop APIs exclusively in Ogg Vorbis up to 320 kbps.
                Musique delivers genuine, bit-perfect lossless audio (FLAC, ALAC, WAV up to 192 kHz)
                by automatically linking your Spotify catalog to local lossless files or playing them
                directly.
              </div>

              {/* Actions */}
              <div style={{ display: "flex", gap: 10 }}>
                <button
                  type="button"
                  onClick={handlePickFile}
                  style={{
                    flex: 1,
                    padding: "9px 14px",
                    borderRadius: 8,
                    background: "rgba(255, 255, 255, 0.12)",
                    border: "1px solid rgba(255, 255, 255, 0.16)",
                    color: "#fff",
                    fontWeight: 600,
                    fontSize: 12.5,
                    cursor: "pointer",
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    gap: 6,
                  }}
                >
                  📁 Play Local Lossless File...
                </button>

                {onOpenSettings && (
                  <button
                    type="button"
                    onClick={() => {
                      setModalOpen(false);
                      onOpenSettings();
                    }}
                    style={{
                      padding: "9px 14px",
                      borderRadius: 8,
                      background: "transparent",
                      border: "1px solid rgba(255, 255, 255, 0.16)",
                      color: "rgba(255, 255, 255, 0.8)",
                      fontWeight: 600,
                      fontSize: 12.5,
                      cursor: "pointer",
                    }}
                  >
                    Configure Library
                  </button>
                )}
              </div>
            </motion.div>
          </div>
        )}
      </AnimatePresence>
    </>
  );
}
