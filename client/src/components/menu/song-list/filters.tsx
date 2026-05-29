import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useAnalysis } from "@/hooks/use-analysis";
import { useSearch } from "@/hooks/use-search";
import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import { AudioLinesIcon } from "lucide-react";

const DEBOUNCE_MS = 500;

export const Filters = () => {
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const { search, setSearch } = useSearch();
  const { enqueueAll } = useAnalysis();
  const { focus, actionsRef } = useMenuFocus();

  useEffect(() => {
    actionsRef.current.onConfirmAnalyzeAll = () => {
      enqueueAll();
    };

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
    <div className="flex w-full items-center gap-2 pl-8 sm:gap-4 sm:pl-0">
      <Input
        defaultValue={search}
        onChange={({ target: { value } }) => handleChange(value)}
        className="flex-1"
        placeholder="Type to search songs..."
      />
      <Button
        tabIndex={-1}
        variant="outline"
        onClick={enqueueAll}
        data-analyze-all-focus="true"
        className={cn(
          "w-7 px-0 focus-visible:ring-0 focus-visible:border-transparent hover:ring-primary sm:w-auto sm:min-w-28 sm:px-3",
          isAnalyzeAllFocused && "ring-2 ring-primary",
        )}
      >
        <AudioLinesIcon />
        <span className="sr-only sm:not-sr-only">Analyze All</span>
      </Button>
    </div>
  );
};
