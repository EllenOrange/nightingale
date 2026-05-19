import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { LrclibCandidate } from "@/types/LrclibCandidate";
import { formatSeconds } from "@/utils/edit-lyrics";
import type { UseQueryResult } from "@tanstack/react-query";
import { ringFor } from "./parts";

interface LrclibMatchesProps {
  query: UseQueryResult<LrclibCandidate[]>;
  index: number;
  onSelect: (candidate: LrclibCandidate) => void;
  useFocused: boolean;
}

export const LrclibMatches = ({ query, index, onSelect, useFocused }: LrclibMatchesProps) => {
  if (query.isLoading) {
    return <p className="text-muted-foreground">Searching LRCLIB…</p>;
  }

  if (query.isError) {
    const message =
      query.error instanceof Error ? query.error.message : "Failed to load LRCLIB matches";
    return <p className="text-destructive">{message}</p>;
  }

  const candidates = query.data ?? [];
  if (candidates.length === 0) {
    return <p className="text-muted-foreground">No LRCLIB matches found for this song.</p>;
  }

  const safeIndex = Math.min(index, candidates.length - 1);
  const candidate = candidates[safeIndex];

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 rounded-md border border-border bg-card p-3">
      <div className="flex min-w-0 flex-col gap-0.5">
        <span className="truncate text-sm font-medium">{candidate.track_name}</span>
        <span className="truncate text-xs text-muted-foreground">
          {candidate.artist_name}
          {candidate.album_name ? ` • ${candidate.album_name}` : ""} •{" "}
          {formatSeconds(candidate.duration_secs)}
        </span>
        <div className="mt-1 flex items-center gap-2">
          <Badge variant="outline">
            {candidate.lines.length} {candidate.lines.length === 1 ? "line" : "lines"}
          </Badge>
          <Button
            size="xs"
            variant="default"
            onClick={() => onSelect(candidate)}
            className={cn("ml-auto", ringFor(useFocused))}
          >
            Use these
          </Button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto rounded border border-border bg-background">
        <pre className="px-3 py-2 font-mono text-[11px] leading-relaxed whitespace-pre-wrap">
          {candidate.lines.join("\n")}
        </pre>
      </div>
    </div>
  );
};
