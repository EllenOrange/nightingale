import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { useDialog } from "@/hooks/use-dialog";
import { useLyricsEditor } from "@/hooks/use-lyrics-editor";
import { useLrclibCandidates } from "@/queries/use-lyrics";
import type { LrclibCandidate } from "@/types/LrclibCandidate";
import { isEditLyricsDialogMode } from "@/utils/edit-lyrics";
import { useRef, useState } from "react";
import { CarouselNav } from "./carousel-nav";
import { EditLyricsFooter } from "./edit-lyrics-footer";
import { LrclibMatches } from "./lrclib-matches";
import { LyricsEditor } from "./lyrics-editor";
import { ringFor } from "./parts";

export { isEditLyricsDialogMode } from "@/utils/edit-lyrics";

type EditLyricsTab = "edit" | "lrclib";

interface NavLayout {
  stops: number[];
  editorSegment: number | null;
  // Top "header row" containing tabs (slots 0..1) and, on the LRCLIB tab, the
  // carousel arrows (slots 2..3) — all in the same segment so left/right walks
  // across them.
  headerSegment: number | null;
  // Slot offset where the carousel arrows start inside `headerSegment`; null
  // if no arrows in this view.
  arrowSlotStart: number | null;
  useThisSegment: number | null;
  footerSegment: number;
}

function navLayout(showMatchesTab: boolean, activeTab: EditLyricsTab): NavLayout {
  if (!showMatchesTab) {
    return {
      stops: [1, 2],
      editorSegment: 0,
      headerSegment: null,
      arrowSlotStart: null,
      useThisSegment: null,
      footerSegment: 1,
    };
  }

  if (activeTab === "edit") {
    return {
      stops: [2, 1, 2],
      editorSegment: 1,
      headerSegment: 0,
      arrowSlotStart: null,
      useThisSegment: null,
      footerSegment: 2,
    };
  }

  return {
    stops: [4, 1, 2],
    editorSegment: null,
    headerSegment: 0,
    arrowSlotStart: 2,
    useThisSegment: 1,
    footerSegment: 2,
  };
}

export const EditLyricsDialog = () => {
  const { mode, close } = useDialog();
  const editLyricsDialog = isEditLyricsDialogMode(mode) ? mode : null;
  const open = editLyricsDialog !== null;
  const song = editLyricsDialog?.song ?? null;
  const fileHash = song?.file_hash ?? null;

  const containerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const editor = useLyricsEditor({ song, onSaved: close });
  const candidatesQuery = useLrclibCandidates(fileHash);
  const candidateCount = candidatesQuery.data?.length ?? 0;
  const showMatchesTab = candidateCount > 1;

  const [activeTab, setActiveTab] = useState<EditLyricsTab>("edit");
  const [carouselIndex, setCarouselIndex] = useState(0);
  const [lastHash, setLastHash] = useState<string | null>(fileHash);
  if (lastHash !== fileHash) {
    setLastHash(fileHash);
    setActiveTab("edit");
    setCarouselIndex(0);
  }

  const applyCandidate = (candidate: LrclibCandidate) => {
    editor.setText(candidate.lines.join("\n"));
    setActiveTab("edit");
  };

  const layout = navLayout(showMatchesTab, activeTab);

  const { isFocused, focusSegment } = useDialogNav({
    open,
    itemCount: layout.stops.reduce((sum, n) => sum + n, 0),
    stops: layout.stops,
    onBack: close,
    containerRef,
    onAction: (segment, slot, action) => {
      const textarea = textareaRef.current;
      const editingInTextarea = textarea !== null && document.activeElement === textarea;

      if (editingInTextarea) {
        if (action.back) {
          textarea.blur();
        }
        return true;
      }

      if (layout.editorSegment !== null && segment === layout.editorSegment && action.confirm) {
        textarea?.focus();
        return true;
      }

      // Radix TabsTrigger activates on mousedown/keydown but not click, so the
      // hook's default click() is a no-op for the tab slots (0..1). Drive the
      // active tab from our state. Arrow slots (>= 2) fall through to the
      // default click() which works on plain buttons.
      if (
        layout.headerSegment !== null &&
        segment === layout.headerSegment &&
        action.confirm &&
        slot < 2
      ) {
        setActiveTab(slot === 0 ? "edit" : "lrclib");
        return true;
      }

      return false;
    },
  });

  if (!song || !editLyricsDialog) {
    return null;
  }

  const editorFocused = layout.editorSegment !== null && isFocused(layout.editorSegment);
  const focusTab = (slot: number) => {
    if (layout.headerSegment !== null) {
      focusSegment(layout.headerSegment, slot);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) close();
      }}
    >
      <DialogContent className="flex h-[85vh] flex-col sm:max-w-2xl">
        <div ref={containerRef} className="contents">
          <DialogHeader>
            <DialogTitle>Edit lyrics</DialogTitle>
            <DialogDescription>
              Each line in the editor becomes a karaoke line after realignment. Saving re-runs
              alignment with your edits.
            </DialogDescription>
          </DialogHeader>

          {showMatchesTab ? (
            <Tabs
              value={activeTab}
              onValueChange={(v) => setActiveTab(v as EditLyricsTab)}
              className="flex min-h-0 flex-1 flex-col"
            >
              <div className="flex items-center justify-between gap-2">
                <TabsList>
                  <TabsTrigger
                    value="edit"
                    onMouseEnter={() => focusTab(0)}
                    onPointerDown={() => focusTab(0)}
                    onFocus={() => focusTab(0)}
                    className={ringFor(
                      layout.headerSegment !== null && isFocused(layout.headerSegment, 0),
                    )}
                  >
                    Edit
                  </TabsTrigger>
                  <TabsTrigger
                    value="lrclib"
                    onMouseEnter={() => focusTab(1)}
                    onPointerDown={() => focusTab(1)}
                    onFocus={() => focusTab(1)}
                    className={ringFor(
                      layout.headerSegment !== null && isFocused(layout.headerSegment, 1),
                    )}
                  >
                    LRCLIB matches ({candidateCount})
                  </TabsTrigger>
                </TabsList>
                {activeTab === "lrclib" &&
                  layout.headerSegment !== null &&
                  layout.arrowSlotStart !== null && (
                    <CarouselNav
                      index={carouselIndex}
                      total={candidateCount}
                      onChange={setCarouselIndex}
                      isFocused={(slot) =>
                        isFocused(
                          layout.headerSegment as number,
                          (layout.arrowSlotStart as number) + slot,
                        )
                      }
                    />
                  )}
              </div>

              <TabsContent value="edit" className="mt-3 flex min-h-0 flex-1 flex-col">
                <LyricsEditor
                  textareaRef={textareaRef}
                  text={editor.text}
                  onChange={editor.setText}
                  disabled={editor.loadingInitial || editor.saving}
                  loadingInitial={editor.loadingInitial}
                  lineCount={editor.normalized.length}
                  isDirty={editor.isDirty}
                  focused={editorFocused}
                />
              </TabsContent>

              <TabsContent value="lrclib" className="mt-3 flex min-h-0 flex-1 flex-col">
                <LrclibMatches
                  query={candidatesQuery}
                  index={carouselIndex}
                  onSelect={applyCandidate}
                  useFocused={layout.useThisSegment !== null && isFocused(layout.useThisSegment)}
                />
              </TabsContent>
            </Tabs>
          ) : (
            <div className="flex min-h-0 flex-1 flex-col">
              <LyricsEditor
                textareaRef={textareaRef}
                text={editor.text}
                onChange={editor.setText}
                disabled={editor.loadingInitial || editor.saving}
                loadingInitial={editor.loadingInitial}
                lineCount={editor.normalized.length}
                isDirty={editor.isDirty}
                focused={editorFocused}
              />
            </div>
          )}

          <EditLyricsFooter
            onCancel={close}
            onSave={editor.handleSave}
            saving={editor.saving}
            canSave={editor.canSave}
            isFocused={(slot) => isFocused(layout.footerSegment, slot)}
          />
        </div>
      </DialogContent>
    </Dialog>
  );
};
