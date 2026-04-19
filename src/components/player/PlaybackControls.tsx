import React from "react";
import { useTranslation } from "react-i18next";
import { Play, Pause, Volume2, VolumeX, Eye, EyeOff, Loader2, SkipBack, Rewind } from "lucide-react";

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}

const PLAYBACK_RATES = [0.5, 0.75, 1, 1.25, 1.5, 2];

interface PlaybackControlsProps {
  currentTime: number;
  duration: number;
  isPlaying: boolean;
  volume: number;
  playbackRate: number;
  previewEdits: boolean;
  previewCacheState: "idle" | "loading" | "ready" | "error";
  previewToggleLabel: string;
  previewCacheModeLabel: string;
  hasWords: boolean;
  onTogglePlay: () => void;
  onRestart: () => void;
  onRewind: () => void;
  onSeekBarChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onToggleMute: () => void;
  onVolumeChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onRateChange: (e: React.ChangeEvent<HTMLSelectElement>) => void;
  onTogglePreviewEdits: () => void;
}

const PlaybackControls: React.FC<PlaybackControlsProps> = React.memo(({
  currentTime,
  duration,
  isPlaying,
  volume,
  playbackRate,
  previewEdits,
  previewCacheState,
  previewToggleLabel,
  previewCacheModeLabel,
  hasWords,
  onTogglePlay,
  onRestart,
  onRewind,
  onSeekBarChange,
  onToggleMute,
  onVolumeChange,
  onRateChange,
  onTogglePreviewEdits,
}) => {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-2 px-3 py-2">
      {/* Seek bar */}
      <input
        type="range"
        min={0}
        max={duration || 0}
        step={0.01}
        value={currentTime}
        onChange={onSeekBarChange}
        className="w-full h-1 appearance-none bg-neutral-700 rounded cursor-pointer accent-logo-primary"
        aria-label="Seek"
      />

      {/* Controls row */}
      <div className="flex items-center gap-3 text-neutral-300">
        {/* Restart */}
        <button
          onClick={onRestart}
          className="hover:text-logo-primary transition-colors"
          aria-label={t("player.restart")}
        >
          <SkipBack size={18} />
        </button>

        {/* Rewind 5s */}
        <button
          onClick={onRewind}
          className="hover:text-logo-primary transition-colors"
          aria-label={t("player.rewind")}
        >
          <Rewind size={18} />
        </button>

        {/* Play/Pause */}
        <button
          onClick={onTogglePlay}
          className="hover:text-logo-primary transition-colors"
          aria-label={isPlaying ? t("player.pause") : t("player.play")}
        >
          {isPlaying ? <Pause size={20} /> : <Play size={20} />}
        </button>

        {/* Time display */}
        <span className="text-xs font-mono tabular-nums min-w-[90px]">
          {formatTime(currentTime)} / {formatTime(duration)}
        </span>

        {/* Preview Edits toggle */}
        {hasWords && (
          <button
            onClick={onTogglePreviewEdits}
            aria-pressed={previewEdits}
            aria-label={previewToggleLabel}
            className={`flex items-center gap-1 text-xs px-2 py-0.5 rounded transition-colors ${
              previewEdits
                ? "text-logo-primary bg-logo-primary/10"
                : "text-neutral-500 hover:text-neutral-300"
            }`}
            title={previewToggleLabel}
          >
            {previewEdits && previewCacheState === "loading" ? (
              <Loader2 size={14} className="animate-spin" />
            ) : previewEdits ? (
              <Eye size={14} />
            ) : (
              <EyeOff size={14} />
            )}
            {previewToggleLabel}
            {previewEdits && (
              <span className="text-[10px] text-neutral-400 ml-1" title={previewCacheModeLabel}>
                {previewCacheModeLabel}
              </span>
            )}
          </button>
        )}

        {/* Spacer */}
        <div className="flex-1" />

        {/* Volume */}
        <button
          onClick={onToggleMute}
          className="hover:text-logo-primary transition-colors"
          aria-label={volume === 0 ? t("player.volume") : t("player.mute")}
        >
          {volume === 0 ? <VolumeX size={18} /> : <Volume2 size={18} />}
        </button>
        <input
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={volume}
          onChange={onVolumeChange}
          className="w-16 h-1 appearance-none bg-neutral-700 rounded cursor-pointer accent-logo-primary"
          aria-label={t("player.volume")}
        />

        {/* Playback speed */}
        <select
          value={playbackRate}
          onChange={onRateChange}
          className="bg-neutral-800 text-neutral-300 text-xs rounded px-1.5 py-0.5 border border-neutral-700 cursor-pointer focus:outline-none focus:border-logo-primary"
          aria-label={t("player.speed")}
        >
          {PLAYBACK_RATES.map((rate) => (
            <option key={rate} value={rate}>
              {/* eslint-disable-next-line i18next/no-literal-string */}
              {rate}x
            </option>
          ))}
        </select>
      </div>
    </div>
  );
});

PlaybackControls.displayName = "PlaybackControls";

export default PlaybackControls;
