import { create } from "zustand";

/**
 * Media metadata returned by the backend after a file is imported. Drives UI
 * labels (file name, type icon) and export defaults (mirrors the source
 * container unless the user overrides in Advanced settings).
 */
interface MediaInfo {
  path: string;
  file_name: string;
  file_size: number;
  media_type: "Video" | "Audio";
  extension: string;
}

/**
 * Global media-player state shared between the transcript editor, the
 * `<video>`/`<audio>` element, and the waveform.
 *
 * Keep this store presentation-only: it mirrors what the player element
 * reports (currentTime/duration/isPlaying) and never performs edits or
 * calls the backend. Anything that mutates the transcript belongs in
 * `editorStore`; anything that affects export belongs in `settingsStore`.
 *
 * `seekVersion` + `seekTarget` form a bump-counter pattern: React components
 * observe `seekVersion` changes to re-issue an imperative `.currentTime = x`
 * on the underlying media element without needing a direct ref.
 */
interface PlayerStore {
  mediaUrl: string | null;
  mediaType: "video" | "audio" | null;
  mediaInfo: MediaInfo | null;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  volume: number;
  playbackRate: number;

  // Incremented to signal the MediaPlayer to perform a seek
  seekVersion: number;
  seekTarget: number;

  setMedia: (url: string, type: "video" | "audio") => void;
  setMediaInfo: (info: MediaInfo | null) => void;
  clearMedia: () => void;
  setPlaying: (playing: boolean) => void;
  setCurrentTime: (time: number) => void;
  setDuration: (duration: number) => void;
  setVolume: (volume: number) => void;
  setPlaybackRate: (rate: number) => void;
  seekTo: (time: number) => void;
}

export const usePlayerStore = create<PlayerStore>()((set) => ({
  mediaUrl: null,
  mediaType: null,
  mediaInfo: null,
  isPlaying: false,
  currentTime: 0,
  duration: 0,
  volume: 1,
  playbackRate: 1,
  seekVersion: 0,
  seekTarget: 0,

  setMedia: (url, type) =>
    set({
      mediaUrl: url,
      mediaType: type,
      isPlaying: false,
      currentTime: 0,
      duration: 0,
    }),

  setMediaInfo: (info) => set({ mediaInfo: info }),

  clearMedia: () =>
    set({
      mediaUrl: null,
      mediaType: null,
      mediaInfo: null,
      isPlaying: false,
      currentTime: 0,
      duration: 0,
    }),

  setPlaying: (playing) => set({ isPlaying: playing }),
  setCurrentTime: (time) => set({ currentTime: time }),
  setDuration: (duration) => set({ duration }),
  setVolume: (volume) => set({ volume: Math.max(0, Math.min(1, volume)) }),
  setPlaybackRate: (rate) => set({ playbackRate: rate }),

  seekTo: (time) =>
    set((state) => ({
      seekTarget: time,
      seekVersion: state.seekVersion + 1,
    })),
}));
