/**
 * Loads the transcript for the current track and normalizes segments for display.
 */

import { loadTranscript } from "@/bridge/playback";
import type { Segment, Transcript } from "@/types/Transcript";
import { useEffect, useState } from "react";
import { splitLongSegments } from "@/utils/playback/transcript-segments";

export function usePlaybackTranscript(fileHash: string) {
  const [segments, setSegments] = useState<Segment[]>([]);
  const [transcriptSource, setTranscriptSource] = useState("generated");

  useEffect(() => {
    loadTranscript(fileHash).then((transcript: Transcript) => {
      setSegments(splitLongSegments(transcript.segments));
      setTranscriptSource(transcript.source ?? "generated");
    });
  }, [fileHash]);

  return { segments, transcriptSource };
}
