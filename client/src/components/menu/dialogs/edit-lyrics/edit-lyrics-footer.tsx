import { Button } from "@/components/ui/button";
import { DialogFooter } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { ARIA_DISABLED_CLASS, ringFor } from "./parts";

interface EditLyricsFooterProps {
  onCancel: () => void;
  onSave: () => void;
  saving: boolean;
  canSave: boolean;
  isFocused: (slot: number) => boolean;
}

export const EditLyricsFooter = ({
  onCancel,
  onSave,
  saving,
  canSave,
  isFocused,
}: EditLyricsFooterProps) => {
  const cancelFocused = isFocused(0);
  const saveFocused = isFocused(1);
  return (
    <DialogFooter>
      <Button
        variant="outline"
        onClick={() => {
          if (saving) return;
          onCancel();
        }}
        aria-disabled={saving}
        className={cn(ARIA_DISABLED_CLASS, ringFor(cancelFocused))}
      >
        Cancel
      </Button>
      <Button
        onClick={() => {
          if (!canSave) return;
          onSave();
        }}
        aria-disabled={!canSave}
        className={cn(ARIA_DISABLED_CLASS, ringFor(saveFocused))}
      >
        {saving ? "Saving…" : "Save & realign"}
      </Button>
    </DialogFooter>
  );
};
