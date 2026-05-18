/**
 * Drives the end-of-song result dialog: watches transport.isFinished + the
 * skip-outro pending flag, persists the run's score to the active profile,
 * plays the success chime, and exposes the props the result dialog needs.
 */

import successSoundUrl from "@/assets/sounds/success.mp3";
import {
  usePlaybackMicState,
  usePlaybackTranscriptActions,
  usePlaybackTranscriptState,
  usePlaybackTransportActions,
  usePlaybackTransportState,
} from "@/contexts/playback";
import { PROFILES } from "@/queries/keys";
import { useProfiles } from "@/queries/use-profiles";
import { addScore } from "@/bridge/profile";
import type { ScoreRecord } from "@/types/ScoreRecord";
import type { Song } from "@/types/Song";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { toast } from "sonner";

export interface PlaybackResult {
  open: boolean;
  score: number;
  scores: ScoreRecord[];
  activeProfile: string | null;
  onFinish: () => void;
}

export function usePlaybackResult(song: Song): PlaybackResult {
  const fileHash = song.file_hash;
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: profileData, isLoading: profilesLoading } = useProfiles();

  const { isFinished } = usePlaybackTransportState();
  const { handleExit } = usePlaybackTransportActions();
  const { rawScore } = usePlaybackMicState();
  const { skipOutroPending } = usePlaybackTranscriptState();
  const { clearSkipOutroPending } = usePlaybackTranscriptActions();

  const [showResult, setShowResult] = useState(false);
  const [resultScore, setResultScore] = useState(0);

  const scoreRef = useRef(rawScore);
  scoreRef.current = rawScore;
  const finishHandledRef = useRef(false);

  useEffect(() => {
    if (!isFinished && !skipOutroPending) {
      return;
    }

    if (profilesLoading && !profileData) {
      return;
    }

    if (finishHandledRef.current) {
      return;
    }

    finishHandledRef.current = true;
    clearSkipOutroPending();

    const finalScore = scoreRef.current;
    const active = profileData?.active ?? null;
    const shouldShowResult = finalScore > 0;

    if (!shouldShowResult) {
      navigate("/", { replace: true });
      return;
    }

    void (async () => {
      try {
        if (active != null) {
          await addScore(fileHash, finalScore);
          await queryClient.invalidateQueries({ queryKey: PROFILES });
        }
        setResultScore(finalScore);
        setShowResult(true);
      } catch (e) {
        toast.error(`Could not save score: ${e instanceof Error ? e.message : String(e)}`);
        navigate("/", { replace: true });
      }
    })();
  }, [
    isFinished,
    skipOutroPending,
    fileHash,
    navigate,
    profileData,
    profilesLoading,
    queryClient,
    clearSkipOutroPending,
  ]);

  useEffect(() => {
    if (!showResult) {
      return;
    }

    const audioEl = new Audio(successSoundUrl);
    void audioEl.play().catch(() => {});

    return () => {
      audioEl.pause();
      audioEl.src = "";
    };
  }, [showResult]);

  const onFinish = useCallback(() => {
    setShowResult(false);
    handleExit();
  }, [handleExit]);

  return {
    open: showResult,
    score: resultScore,
    scores: profileData?.scores ?? [],
    activeProfile: profileData?.active ?? null,
    onFinish,
  };
}
