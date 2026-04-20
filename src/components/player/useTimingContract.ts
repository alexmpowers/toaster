import { useEffect, useMemo, useRef, useState } from "react";
import { commands } from "@/bindings";
import {
  getDeletedRanges,
  getDeletedRangesFromKeepSegments,
  type TimeSegment,
} from "@/lib/utils/timeline";
import type { useEditorStore } from "@/stores/editorStore";

type TimingContract = ReturnType<
  typeof useEditorStore.getState
>["timingContract"];

interface UseTimingContractParams {
  words: ReturnType<typeof useEditorStore.getState>["words"];
  duration: number;
  timingContract: TimingContract;
}

/**
 * Maps the backend timing contract (or a `commands.getKeepSegments()` fallback)
 * into the deleted-range / keep-segment projections that the player consumes.
 *
 * Per the dual-path single-source-of-truth rule, this hook does not derive
 * keep-segments from frontend state; it only normalizes backend-supplied data
 * (or falls back to the local heuristic when the backend is unavailable).
 */
export function useTimingContract({
  words,
  duration,
  timingContract,
}: UseTimingContractParams): {
  activeDeletedRanges: TimeSegment[];
  backendKeepSegments: TimeSegment[];
} {
  const [backendDeletedRanges, setBackendDeletedRanges] = useState<
    TimeSegment[] | null
  >(null);
  const [backendKeepSegments, setBackendKeepSegments] = useState<TimeSegment[]>(
    [],
  );
  const backendFetchSeq = useRef(0);

  const deletedRanges = useMemo(
    () => getDeletedRanges(words, duration),
    [words, duration],
  );
  const activeDeletedRanges = backendDeletedRanges ?? deletedRanges;

  useEffect(() => {
    let isCancelled = false;
    const seq = ++backendFetchSeq.current;

    if (words.length === 0) {
      setBackendDeletedRanges([]);
      setBackendKeepSegments([]);
      return;
    }

    if (timingContract) {
      const keepSegments =
        timingContract.keep_segments?.length > 0
          ? timingContract.keep_segments
          : (timingContract.quantized_keep_segments ?? []);
      setBackendDeletedRanges(
        getDeletedRangesFromKeepSegments(words, keepSegments),
      );
      const normalized = keepSegments
        .map((s) => ({
          start: s.start_us / 1_000_000,
          end: s.end_us / 1_000_000,
        }))
        .filter((s) => s.end > s.start)
        .sort((a, b) => a.start - b.start);
      setBackendKeepSegments(normalized);

      if (!timingContract.keep_segments_valid && timingContract.warning) {
        console.warn(
          `[timing-contract] revision=${timingContract.timeline_revision} warning=${timingContract.warning}`,
        );
      }
      return;
    }

    const refreshKeepSegments = async () => {
      try {
        const result = await commands.getKeepSegments();
        if (isCancelled || seq !== backendFetchSeq.current) return;
        if (result.status === "ok") {
          setBackendDeletedRanges(
            getDeletedRangesFromKeepSegments(words, result.data),
          );
          const normalized = result.data
            .map((s) => ({
              start: s.start_us / 1_000_000,
              end: s.end_us / 1_000_000,
            }))
            .filter((s) => s.end > s.start)
            .sort((a, b) => a.start - b.start);
          setBackendKeepSegments(normalized);
          return;
        }
      } catch {
        // Fallback to local deleted-ranges heuristic below
      }

      if (!isCancelled && seq === backendFetchSeq.current) {
        setBackendDeletedRanges(null);
        setBackendKeepSegments([]);
      }
    };

    void refreshKeepSegments();
    return () => {
      isCancelled = true;
    };
  }, [words, timingContract]);

  return { activeDeletedRanges, backendKeepSegments };
}
