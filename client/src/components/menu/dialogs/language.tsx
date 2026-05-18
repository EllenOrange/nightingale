import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup } from "@/components/ui/field";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { DialogMode, useDialog } from "@/hooks/use-dialog";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { reanalyzeTranscript } from "@/bridge/analysis";
import { Song } from "@/types/Song";

const LANGUAGES = [
  ["en", "English"],
  ["es", "Spanish"],
  ["fr", "French"],
  ["de", "German"],
  ["it", "Italian"],
  ["pt", "Portuguese"],
  ["ru", "Russian"],
  ["ja", "Japanese"],
  ["ko", "Korean"],
  ["zh", "Chinese"],
  ["ar", "Arabic"],
  ["hi", "Hindi"],
  ["nl", "Dutch"],
  ["pl", "Polish"],
  ["sv", "Swedish"],
  ["tr", "Turkish"],
  ["uk", "Ukrainian"],
  ["cs", "Czech"],
  ["ro", "Romanian"],
  ["hu", "Hungarian"],
];

export function isLanguageDialogMode(mode: DialogMode): mode is { mode: "language"; song: Song } {
  return mode !== null && typeof mode === "object" && mode.mode === "language";
}

export const SelectLanguageDialog = () => {
  const { mode, close } = useDialog();
  const containerRef = useRef<HTMLDivElement>(null);

  const languageDialog = isLanguageDialogMode(mode) ? mode : null;
  const open = languageDialog !== null;
  const currentLanguage = languageDialog?.song.language;

  const [language, setLanguage] = useState(currentLanguage);

  const { focusedIndex } = useDialogNav({
    open,
    itemCount: 3,
    onBack: close,
    containerRef,
  });

  if (!languageDialog) {
    return null;
  }

  const { song } = languageDialog;

  if (!song.language) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-sm">
        <div ref={containerRef} className="contents">
          <DialogHeader>
            <DialogTitle>Select Language</DialogTitle>
          </DialogHeader>
          <FieldGroup>
            <Field>
              <Label htmlFor="model-1">Language</Label>
              <Select
                onValueChange={(language) => setLanguage(language)}
                defaultValue={song.language}
              >
                <SelectTrigger
                  id="model-1"
                  className={cn(
                    "focus-visible:ring-0 focus-visible:border-transparent",
                    focusedIndex === 0 && "ring-2 ring-primary",
                  )}
                >
                  <SelectValue placeholder="Select a profile" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectLabel>Profile</SelectLabel>
                    {LANGUAGES.map(([value, label]) => (
                      <SelectItem value={value}>{label}</SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>
          <DialogFooter>
            <DialogClose asChild>
              <Button
                variant="outline"
                onClick={close}
                className={cn(
                  "focus-visible:ring-0 focus-visible:border-transparent",
                  focusedIndex === 1 && "ring-2 ring-primary",
                )}
              >
                Cancel
              </Button>
            </DialogClose>
            <Button
              disabled={language === song.language}
              onClick={() => {
                if (language) {
                  reanalyzeTranscript(song.file_hash, language);
                }

                close();
              }}
              className={cn(
                "focus-visible:ring-0 focus-visible:border-transparent",
                focusedIndex === 2 && "ring-2 ring-primary",
              )}
            >
              Select
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
};
