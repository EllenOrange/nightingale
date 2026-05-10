import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { Separator } from "@/components/ui/separator";
import { Spinner } from "@/components/ui/spinner";
import { useDialog } from "@/hooks/use-dialog";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { cn } from "@/lib/utils";
import { useUpdate } from "@/queries/use-update";
import { downloadAndInstallUpdate, relaunchApp } from "@/tauri-bridge/updater";
import { CheckCircle2Icon, DownloadIcon, WifiOffIcon } from "lucide-react";
import prettyBytes from "pretty-bytes";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { version as currentVersion } from "../../../../package.json";

const RING = "ring-2 ring-primary";
const NO_FOCUS_RING = "focus-visible:ring-0 focus-visible:border-transparent";

type InstallStage = "idle" | "downloading" | "installing" | "finished" | "install-error";

interface DownloadProgress {
  contentLength: number | null;
  downloaded: number;
}

const formatPubDate = (date: string | undefined): string | null => {
  if (!date) {
    return null;
  }

  try {
    const parsed = new Date(date);
    if (Number.isNaN(parsed.getTime())) {
      return null;
    }
    return parsed.toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return null;
  }
};

export const UpdateDialog = () => {
  const { mode, close } = useDialog();
  const open = mode === "update";

  const { status, update, error, isOffline, refetch } = useUpdate();

  const [installStage, setInstallStage] = useState<InstallStage>("idle");
  const [installError, setInstallError] = useState<string | null>(null);
  const [progress, setProgress] = useState<DownloadProgress>({
    contentLength: null,
    downloaded: 0,
  });

  const containerRef = useRef<HTMLDivElement>(null);
  const wasOpenRef = useRef(false);

  useEffect(() => {
    if (open && !wasOpenRef.current && installStage === "idle") {
      refetch();
    }
    wasOpenRef.current = open;
  }, [open, installStage, refetch]);

  useEffect(() => {
    if (!open) {
      setInstallStage("idle");
      setInstallError(null);
      setProgress({ contentLength: null, downloaded: 0 });
    }
  }, [open]);

  const isBusy = installStage === "downloading" || installStage === "installing";

  const safeClose = () => {
    if (isBusy) {
      return;
    }
    close();
  };

  const handleInstall = async () => {
    if (!update) {
      return;
    }

    setInstallStage("downloading");
    setInstallError(null);
    setProgress({ contentLength: null, downloaded: 0 });

    try {
      await downloadAndInstallUpdate(update, (event) => {
        if (event.event === "Started") {
          setProgress({
            contentLength: event.data.contentLength ?? null,
            downloaded: 0,
          });
        } else if (event.event === "Progress") {
          setProgress((prev) => ({
            ...prev,
            downloaded: prev.downloaded + event.data.chunkLength,
          }));
        } else if (event.event === "Finished") {
          setInstallStage("installing");
        }
      });
      setInstallStage("finished");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      setInstallError(message);
      setInstallStage("install-error");
      toast.error(`Update failed: ${message}`);
    }
  };

  const handleRestart = async () => {
    try {
      await relaunchApp();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      toast.error(`Restart failed: ${message}`);
    }
  };

  let itemCount = 0;
  if (isBusy) {
    itemCount = 0;
  } else if (installStage === "finished") {
    itemCount = 1;
  } else if (installStage === "install-error") {
    itemCount = 2;
  } else if (status === "checking") {
    itemCount = 1;
  } else if (status === "error") {
    itemCount = 2;
  } else if (status === "up-to-date") {
    itemCount = 2;
  } else if (status === "available") {
    itemCount = 2;
  }

  const { focusedIndex } = useDialogNav({
    open,
    itemCount,
    onBack: isBusy ? () => {} : close,
    containerRef,
  });

  const ringFor = (idx: number) => cn(NO_FOCUS_RING, open && focusedIndex === idx && RING);

  const renderBody = () => {
    if (installStage === "downloading") {
      const { contentLength, downloaded } = progress;
      const percent =
        contentLength && contentLength > 0
          ? Math.min(100, Math.floor((downloaded / contentLength) * 100))
          : null;
      return (
        <div className="flex flex-col gap-3">
          <p className="text-xs text-muted-foreground">
            Downloading {update?.version ?? "update"}…
          </p>
          <Progress value={percent ?? 0} max={100} />
          <p className="text-xs text-muted-foreground">
            {contentLength
              ? `${prettyBytes(downloaded)} of ${prettyBytes(contentLength)}${
                  percent !== null ? ` (${percent}%)` : ""
                }`
              : prettyBytes(downloaded)}
          </p>
        </div>
      );
    }

    if (installStage === "installing") {
      return (
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Spinner className="size-4" />
          <span>Installing update… The app will restart shortly.</span>
        </div>
      );
    }

    if (installStage === "finished") {
      return (
        <div className="flex items-start gap-2 text-xs text-muted-foreground">
          <CheckCircle2Icon className="size-4 text-chart-3 shrink-0 mt-0.5" />
          <span>
            Update installed. Restart now to start using version{" "}
            {update?.version ?? "the new build"}.
          </span>
        </div>
      );
    }

    if (installStage === "install-error") {
      return (
        <div className="flex flex-col gap-2 text-xs">
          <p className="text-destructive">The update could not be installed.</p>
          {installError && <p className="text-muted-foreground break-words">{installError}</p>}
        </div>
      );
    }

    if (status === "checking") {
      return (
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Spinner className="size-4" />
          <span>Checking for updates…</span>
        </div>
      );
    }

    if (status === "error") {
      const Icon = isOffline ? WifiOffIcon : null;

      const headline = isOffline
        ? "Couldn't reach the update server."
        : "Couldn't check for updates.";

      const hint = isOffline
        ? "Check your internet connection and try again."
        : (error?.message ?? "Please try again later.");

      return (
        <div className="flex items-start gap-2 text-xs">
          {Icon && <Icon className="size-4 text-destructive shrink-0 mt-0.5" />}
          <div className="flex flex-col gap-1">
            <p className="text-destructive">{headline}</p>
            <p className="text-muted-foreground break-words">{hint}</p>
          </div>
        </div>
      );
    }

    if (status === "up-to-date") {
      return (
        <div className="flex items-start gap-2 text-xs text-muted-foreground">
          <CheckCircle2Icon className="size-4 text-chart-3 shrink-0 mt-0.5" />
          <span>You're on the latest version (v{currentVersion}).</span>
        </div>
      );
    }

    if (status === "available" && update) {
      const pubDate = formatPubDate(update.date);

      const notes = update.body?.trim();

      return (
        <div className="flex flex-col gap-3">
          <div className="flex items-start gap-2">
            <DownloadIcon className="size-4 text-primary shrink-0 mt-0.5" />
            <div className="flex flex-col gap-0.5">
              <p className="text-xs">
                <span className="font-medium">Version {update.version}</span> is available
              </p>
              <p className="text-[0.7rem] text-muted-foreground">
                You're on v{currentVersion}
                {pubDate ? ` · Released ${pubDate}` : ""}
              </p>
            </div>
          </div>
          {notes && (
            <>
              <Separator />
              <div className="flex flex-col gap-1">
                <h4 className="text-[0.65rem] font-medium uppercase tracking-wide text-muted-foreground">
                  Release notes
                </h4>
                <p className="text-xs text-muted-foreground whitespace-pre-wrap break-words max-h-48 overflow-y-auto scrollbar-hide">
                  {notes}
                </p>
              </div>
            </>
          )}
        </div>
      );
    }

    return null;
  };

  const renderFooter = () => {
    if (isBusy) {
      return null;
    }

    if (installStage === "finished") {
      return (
        <DialogFooter>
          <Button onClick={handleRestart} className={ringFor(0)}>
            Restart now
          </Button>
        </DialogFooter>
      );
    }

    if (installStage === "install-error") {
      return (
        <DialogFooter>
          <Button variant="outline" onClick={close} className={ringFor(0)}>
            Close
          </Button>
          <Button onClick={handleInstall} className={ringFor(1)}>
            Retry install
          </Button>
        </DialogFooter>
      );
    }

    if (status === "checking") {
      return (
        <DialogFooter>
          <Button variant="outline" onClick={close} className={ringFor(0)}>
            Close
          </Button>
        </DialogFooter>
      );
    }

    if (status === "error") {
      return (
        <DialogFooter>
          <Button variant="outline" onClick={close} className={ringFor(0)}>
            Close
          </Button>
          <Button onClick={() => refetch()} className={ringFor(1)}>
            Retry
          </Button>
        </DialogFooter>
      );
    }

    if (status === "up-to-date") {
      return (
        <DialogFooter>
          <Button variant="outline" onClick={close} className={ringFor(0)}>
            Close
          </Button>
          <Button onClick={() => refetch()} className={ringFor(1)}>
            Check again
          </Button>
        </DialogFooter>
      );
    }

    if (status === "available") {
      return (
        <DialogFooter>
          <Button variant="outline" onClick={close} className={ringFor(0)}>
            Later
          </Button>
          <Button onClick={handleInstall} className={ringFor(1)}>
            Install &amp; Restart
          </Button>
        </DialogFooter>
      );
    }

    return null;
  };

  return (
    <Dialog open={open} onOpenChange={(next) => (next ? undefined : safeClose())}>
      <DialogContent
        className="sm:max-w-md"
        showCloseButton={!isBusy}
        onEscapeKeyDown={(e) => {
          if (isBusy) {
            e.preventDefault();
          }
        }}
        onInteractOutside={(e) => {
          if (isBusy) {
            e.preventDefault();
          }
        }}
      >
        <div ref={containerRef} className="contents">
          <DialogHeader>
            <DialogTitle>Update</DialogTitle>
            <DialogDescription>
              {status === "available" && installStage === "idle"
                ? "A new version of Nightingale is available."
                : "Keep Nightingale up to date with the latest improvements."}
            </DialogDescription>
          </DialogHeader>
          {renderBody()}
          {renderFooter()}
        </div>
      </DialogContent>
    </Dialog>
  );
};
