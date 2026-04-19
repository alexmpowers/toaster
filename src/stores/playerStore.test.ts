import { describe, it, expect, beforeEach } from "vitest";
import { usePlayerStore } from "./playerStore";

function reset() {
  usePlayerStore.setState({
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
  });
}

describe("playerStore", () => {
  beforeEach(reset);

  describe("setMedia", () => {
    it("sets url + type and resets playback state", () => {
      usePlayerStore.setState({
        isPlaying: true,
        currentTime: 42,
        duration: 100,
      });
      usePlayerStore.getState().setMedia("file:///movie.mp4", "video");
      const s = usePlayerStore.getState();
      expect(s.mediaUrl).toBe("file:///movie.mp4");
      expect(s.mediaType).toBe("video");
      expect(s.isPlaying).toBe(false);
      expect(s.currentTime).toBe(0);
      expect(s.duration).toBe(0);
    });

    it("does not touch volume or playbackRate", () => {
      usePlayerStore.getState().setVolume(0.5);
      usePlayerStore.getState().setPlaybackRate(1.5);
      usePlayerStore.getState().setMedia("file:///a.wav", "audio");
      const s = usePlayerStore.getState();
      expect(s.volume).toBe(0.5);
      expect(s.playbackRate).toBe(1.5);
    });
  });

  describe("clearMedia", () => {
    it("nulls out media refs and stops playback", () => {
      usePlayerStore.getState().setMedia("file:///x.mp4", "video");
      usePlayerStore.setState({ isPlaying: true, currentTime: 10, duration: 20 });
      usePlayerStore.getState().clearMedia();
      const s = usePlayerStore.getState();
      expect(s.mediaUrl).toBeNull();
      expect(s.mediaType).toBeNull();
      expect(s.mediaInfo).toBeNull();
      expect(s.isPlaying).toBe(false);
      expect(s.currentTime).toBe(0);
      expect(s.duration).toBe(0);
    });
  });

  describe("setVolume", () => {
    it("clamps to [0, 1]", () => {
      const { setVolume } = usePlayerStore.getState();
      setVolume(-0.5);
      expect(usePlayerStore.getState().volume).toBe(0);
      setVolume(2.5);
      expect(usePlayerStore.getState().volume).toBe(1);
      setVolume(0.3);
      expect(usePlayerStore.getState().volume).toBeCloseTo(0.3);
    });
  });

  describe("seekTo", () => {
    it("bumps seekVersion and records the new target", () => {
      const before = usePlayerStore.getState().seekVersion;
      usePlayerStore.getState().seekTo(12.5);
      const after = usePlayerStore.getState();
      expect(after.seekTarget).toBe(12.5);
      expect(after.seekVersion).toBe(before + 1);
    });

    it("bumps the version on every call, even with the same target", () => {
      usePlayerStore.getState().seekTo(7);
      const v1 = usePlayerStore.getState().seekVersion;
      usePlayerStore.getState().seekTo(7);
      expect(usePlayerStore.getState().seekVersion).toBe(v1 + 1);
    });
  });

  describe("simple setters", () => {
    it("setPlaying / setCurrentTime / setDuration / setPlaybackRate pass values through", () => {
      const s = usePlayerStore.getState();
      s.setPlaying(true);
      s.setCurrentTime(3.14);
      s.setDuration(60);
      s.setPlaybackRate(2);
      const got = usePlayerStore.getState();
      expect(got.isPlaying).toBe(true);
      expect(got.currentTime).toBe(3.14);
      expect(got.duration).toBe(60);
      expect(got.playbackRate).toBe(2);
    });
  });

  describe("setMediaInfo", () => {
    it("stores the info payload and accepts null to clear", () => {
      const info = {
        path: "C:/x.mp4",
        file_name: "x.mp4",
        file_size: 1234,
        media_type: "Video" as const,
        extension: "mp4",
      };
      usePlayerStore.getState().setMediaInfo(info);
      expect(usePlayerStore.getState().mediaInfo).toEqual(info);
      usePlayerStore.getState().setMediaInfo(null);
      expect(usePlayerStore.getState().mediaInfo).toBeNull();
    });
  });
});
