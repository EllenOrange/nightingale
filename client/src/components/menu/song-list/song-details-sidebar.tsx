import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import type { QueuedStatus } from "@/types/QueuedStatus";
import type { Song } from "@/types/Song";
import { PlayIcon } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router";
import { ActionsSection } from "./details/actions-section";
import { KeyTempoSection } from "./details/key-tempo-section";
import { SongDetailsHeader } from "./details/song-details-header";
import { useSongDetailsNav } from "./details/use-song-details-nav";
import { getSongStatusInfo } from "./shared/song-status";
import type { ShiftType } from "./shifts";

interface SongDetailsSidebarProps {
  song: Song;
  queueStatus?: QueuedStatus;
  onClose: () => void;
}

export const SongDetailsSidebar = ({ song, queueStatus, onClose }: SongDetailsSidebarProps) => {
  const navigate = useNavigate();
  const { detailsRef, closeDetails } = useSongDetailsNav(onClose);
  const [shifting, setShifting] = useState<Record<ShiftType, boolean>>({
    tempo: false,
    key: false,
  });

  const status = getSongStatusInfo(song.is_analyzed, queueStatus);
  const analysisBusy = queueStatus === "Queued" || Boolean(status.isAnalyzing);
  const supportsShifts = song.is_analyzed && song.transcript_source !== "Usdx";
  const supportsAnalysisActions = Boolean(status.isReady) && song.transcript_source !== "Usdx";

  return (
    <aside
      ref={detailsRef}
      className="flex min-h-0 min-w-0 flex-1 flex-col border-l bg-background [&_[data-song-details-focused=true]]:z-10 [&_[data-song-details-focused=true]]:ring-2 [&_[data-song-details-focused=true]]:ring-primary xl:w-96 xl:flex-none"
      aria-label="Song details"
    >
      <SongDetailsHeader song={song} queueStatus={queueStatus} onClose={closeDetails} />

      <div className="no-scrollbar min-h-0 flex-1 overflow-y-auto">
        <KeyTempoSection
          song={song}
          supportsShifts={supportsShifts}
          shifting={shifting}
          setShifting={setShifting}
        />

        <Separator />

        <ActionsSection
          song={song}
          status={status}
          analysisBusy={analysisBusy}
          supportsAnalysisActions={supportsAnalysisActions}
        />
      </div>

      <footer className="border-t p-3 pb-[max(0.75rem,env(safe-area-inset-bottom))]">
        <Button
          size="lg"
          className="h-8 w-full"
          disabled={!status.isReady || shifting.key || shifting.tempo}
          onClick={() => navigate("/playback", { state: { song } })}
        >
          <PlayIcon /> Play
        </Button>
      </footer>
    </aside>
  );
};
