import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useAnalysis } from "@/hooks/use-analysis";
import { useSearch } from "@/hooks/use-search";
import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

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
    <div className="flex w-full items-center gap-4">
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
        className={cn(
          "focus-visible:ring-0 focus-visible:border-transparent",
          isAnalyzeAllFocused && "ring-2 ring-primary",
        )}
      >
        Analyze All
      </Button>
    </div>
  );
};
