const SPEAKER_COLORS = [
  "rgb(139 92 246)",
  "rgb(6 182 212)",
  "rgb(249 115 22)",
  "rgb(16 185 129)",
  "rgb(236 72 153)",
  "rgb(99 102 241)",
  "rgb(20 184 166)",
  "rgb(245 158 11)",
] as const;

export function getSpeakerColor(speakerId: number): string {
  if (speakerId < 0) {
    return "transparent";
  }
  return SPEAKER_COLORS[speakerId % SPEAKER_COLORS.length];
}
