import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useAnalysis } from "@/hooks/use-analysis";
import { useSearch } from "@/hooks/use-search";
import { cn } from "@/lib/utils";
import { AudioLinesIcon, Grid2X2Icon, ListIcon } from "lucide-react";
import { useEffect, useRef } from "react";

const DEBOUNCE_MS = 500;
export type SongListView = "table" | "grid";

interface FiltersProps {
  view: SongListView;
  onViewChange: (view: SongListView) => void;
  isSavingView?: boolean;
}

export const Filters = ({ view, onViewChange, isSavingView }: FiltersProps) => {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const { search, setSearch } = useSearch();
  const { enqueueAll } = useAnalysis();
  const { focus, actionsRef } = useMenuFocus();

  useEffect(() => {
    actionsRef.current.onConfirmAnalyzeAll = enqueueAll;
    return () => {
      actionsRef.current.onConfirmAnalyzeAll = null;
    };
  }, [actionsRef, enqueueAll]);

  const handleChange = (value: string) => {
    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => setSearch(value), DEBOUNCE_MS);
  };

  const isAnalyzeAllFocused = focus.active && focus.panel === "songList" && focus.analyzeAllFocused;

  return (
    <div className="flex w-full items-center gap-2 pl-8 sm:pl-0">
      <Input
        defaultValue={search}
        onChange={({ target: { value } }) => handleChange(value)}
        className="min-w-0 flex-1 sm:w-72 sm:flex-none"
        placeholder="Search songs"
        aria-label="Search songs"
      />
      <Button
        tabIndex={-1}
        variant="outline"
        onClick={enqueueAll}
        data-analyze-all-focus="true"
        className={cn(
          "ml-auto w-7 px-0 focus-visible:border-transparent focus-visible:ring-0 sm:w-auto sm:min-w-28 sm:px-3",
          isAnalyzeAllFocused && "ring-2 ring-primary",
        )}
      >
        <AudioLinesIcon />
        <span className="sr-only sm:not-sr-only">Analyze all</span>
      </Button>
      <div
        className="flex rounded-md border bg-input/20 p-0.5"
        role="group"
        aria-label="Song list view"
      >
        <Button
          variant={view === "table" ? "secondary" : "ghost"}
          size="icon-sm"
          disabled={isSavingView}
          onClick={() => onViewChange("table")}
          aria-label="Table view"
          aria-pressed={view === "table"}
          title="Table view"
        >
          <ListIcon />
        </Button>
        <Button
          variant={view === "grid" ? "secondary" : "ghost"}
          size="icon-sm"
          disabled={isSavingView}
          onClick={() => onViewChange("grid")}
          aria-label="Card grid view"
          aria-pressed={view === "grid"}
          title="Card grid view"
        >
          <Grid2X2Icon />
        </Button>
      </div>
    </div>
  );
};
