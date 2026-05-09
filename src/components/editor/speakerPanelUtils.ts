import type { SpeakerInfo } from "@/stores/editorStore";

export function toSpeakerNameMap(
  speakers: SpeakerInfo[],
): Record<string, string> {
  return speakers.reduce<Record<string, string>>((accumulator, speaker) => {
    if (speaker.name.trim().length > 0) {
      accumulator[String(speaker.id)] = speaker.name;
    }
    return accumulator;
  }, {});
}

export function formatDurationSeconds(totalDurationUs: number): string {
  return (totalDurationUs / 1_000_000).toFixed(1);
}
