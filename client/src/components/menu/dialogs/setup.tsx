import { onSetupError, onSetupProgress, triggerSetup } from "@/bridge/setup";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useNavInput } from "@/hooks/navigation/use-nav-input";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { exit, EXIT_SUPPORTED } from "@/bridge/exit";
import { Progress } from "@/components/ui/progress";
import type { SetupProgress } from "@/types/SetupProgress";
import type { SetupStep } from "@/types/SetupStep";
import logoSrc from "@/assets/images/logo_square.png";
import { useShouldRunSetup } from "@/hooks/use-should-run-setup";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { selectFolderRaw } from "@/bridge/folder";
import { useConfig } from "@/queries/use-config";
import { ANALYSIS_QUEUE, CONFIG, MENU, SONGS, SONGS_META } from "@/queries/keys";
import { useQueryClient } from "@tanstack/react-query";

interface ExtendedSetupProgress extends Omit<SetupProgress, "step"> {
  step: SetupStep | "init" | "error" | "changedatafolder";
}

type InitialStepProps = {
  toNextStep: () => void;
};

const InitialStep = ({ toNextStep }: InitialStepProps) => {
  return (
    <>
      <AlertDialogHeader>
        <AlertDialogTitle>Welcome to Nightingale!</AlertDialogTitle>
        <AlertDialogDescription>
          Before you get started, we need to install a few dependencies: <code>ffmpeg</code>,{" "}
          <code>uv</code>, <code>python 3.10</code>, Python packages, and <code>CUDA</code> wheels
          (NVIDIA GPUs only).
        </AlertDialogDescription>
        <AlertDialogDescription>
          This may take a few minutes.{" "}
          {EXIT_SUPPORTED ? "You can exit at any time if you'd prefer not to continue." : ""}
        </AlertDialogDescription>
        <AlertDialogDescription>This only happens once.</AlertDialogDescription>
      </AlertDialogHeader>
      <AlertDialogFooter>
        {EXIT_SUPPORTED && <AlertDialogCancel onClick={() => exit()}>Exit</AlertDialogCancel>}
        <AlertDialogAction onClick={toNextStep}>Continue</AlertDialogAction>
      </AlertDialogFooter>
    </>
  );
};

type ChangeDataStepProps = {
  onStart: () => Promise<void>;
  folder?: string;
  setFolder: (folder?: string) => void;
};

const ChangeDataFolderStep = ({ onStart, folder, setFolder }: ChangeDataStepProps) => (
  <>
    <AlertDialogHeader>
      <AlertDialogTitle>Data Folder</AlertDialogTitle>
      <>
        <AlertDialogDescription className="mb-2">
          Choose where Nightingale stores app data. We will store cache, videos, models, vendor
          tools, and the library database in this folder. Only <code>config.json</code> and{" "}
          <code>nightingale.log</code> stay in the default <code>~/.nightingale</code> path.
        </AlertDialogDescription>
        <div className="flex gap-2 w-full">
          <Input value={folder ?? ""} disabled />
          <Button
            variant="outline"
            onClick={async () => {
              const folder = await selectFolderRaw();

              if (!folder) {
                return;
              }

              setFolder(folder);
            }}
          >
            {folder ? "Change Folder" : "Choose Folder"}
          </Button>
        </div>
      </>
    </AlertDialogHeader>
    <AlertDialogFooter>
      {EXIT_SUPPORTED && <AlertDialogCancel onClick={() => exit()}>Exit</AlertDialogCancel>}
      <AlertDialogAction onClick={onStart}>Continue</AlertDialogAction>
    </AlertDialogFooter>
  </>
);

interface LoadStepProps {
  action: string;
  percent: number;
}

const LoadStep = ({ action, percent }: LoadStepProps) => (
  <>
    <AlertDialogHeader>
      <AlertDialogTitle>Setting up Nightingale</AlertDialogTitle>
      <div className="flex flex-col gap-2 w-full">
        <AlertDialogDescription className="w-full">{action}</AlertDialogDescription>
        <Progress value={percent} />
      </div>
    </AlertDialogHeader>
    {EXIT_SUPPORTED && (
      <AlertDialogFooter>
        <AlertDialogCancel onClick={() => exit()}>Exit</AlertDialogCancel>
      </AlertDialogFooter>
    )}
  </>
);

interface ErrorStepProps {
  error: string;
}

const ErrorStep = ({ error }: ErrorStepProps) => (
  <>
    <AlertDialogHeader>
      <AlertDialogTitle>Something went wrong</AlertDialogTitle>
      <AlertDialogDescription>
        <code>{error}</code>
      </AlertDialogDescription>
    </AlertDialogHeader>
    {EXIT_SUPPORTED && (
      <AlertDialogFooter>
        <AlertDialogAction onClick={() => exit()}>Exit</AlertDialogAction>
      </AlertDialogFooter>
    )}
  </>
);

interface FinalStepProps {
  onFinish: () => void;
  folder?: string;
}

const FinalStep = ({ onFinish, folder }: FinalStepProps) => (
  <>
    <AlertDialogHeader>
      <AlertDialogTitle>You're all set!</AlertDialogTitle>
      <AlertDialogDescription>
        All dependencies have been installed to <code>{folder}/vendor</code>. Nightingale is ready
        to use.
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogAction onClick={onFinish}>Get Started</AlertDialogAction>
    </AlertDialogFooter>
  </>
);

const defaultProgress = {
  step: "init" as const,
  percent: 0,
  action: "",
};

export const Setup = () => {
  const { data: config } = useConfig();
  const { shouldRunSetup, setShouldRunSetup } = useShouldRunSetup();
  const queryClient = useQueryClient();

  const [overrideFolder, setOverrideFolder] = useState(config?.data_path);
  const [setupProgress, setSetupProgress] = useState<ExtendedSetupProgress>(defaultProgress);

  useEffect(() => {
    if (!overrideFolder && config?.data_path) {
      setOverrideFolder(config.data_path);
    }
  }, [config?.data_path, overrideFolder]);

  const invalidatePostSetupState = useCallback(async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: CONFIG }),
      queryClient.invalidateQueries({ queryKey: SONGS_META }),
      queryClient.invalidateQueries({ queryKey: SONGS }),
      queryClient.invalidateQueries({ queryKey: MENU }),
      queryClient.invalidateQueries({ queryKey: ANALYSIS_QUEUE }),
    ]);
  }, [queryClient]);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    onSetupProgress((progress) => {
      setSetupProgress(progress);
      if (progress.step === "finish") {
        void invalidatePostSetupState();
      }
    }).then((fn) => {
      unlistenProgress = fn;
    });

    onSetupError((error) => {
      setSetupProgress({ step: "error", percent: 0, action: error });
    }).then((fn) => {
      unlistenError = fn;
    });

    return () => {
      unlistenProgress?.();
      unlistenError?.();
    };
  }, [invalidatePostSetupState]);

  const { step, percent, action } = setupProgress;

  useNavInput(
    useCallback(
      (navAction) => {
        if (!shouldRunSetup) {
          return;
        }

        if (navAction.back) {
          if (step === "finish") {
            setShouldRunSetup(false);
          } else if (EXIT_SUPPORTED) {
            exit();
          }

          return;
        }

        if (navAction.confirm) {
          if (step === "init") {
            triggerSetup(overrideFolder ?? undefined);
          } else if (step === "finish") {
            void invalidatePostSetupState();
            setShouldRunSetup(false);
          } else if (step === "error" && EXIT_SUPPORTED) {
            exit();
          }
        }
      },
      [invalidatePostSetupState, overrideFolder, setShouldRunSetup, shouldRunSetup, step],
    ),
  );

  const Step = useMemo(() => {
    switch (step) {
      case "init":
        return () => (
          <InitialStep
            toNextStep={() => setSetupProgress({ ...setupProgress, step: "changedatafolder" })}
          />
        );
      case "changedatafolder":
        return () => (
          <ChangeDataFolderStep
            folder={overrideFolder ?? undefined}
            setFolder={setOverrideFolder}
            onStart={() => triggerSetup(overrideFolder ?? undefined)}
          />
        );
      case "clearvendor":
      case "ffmpeg":
      case "migratedata":
      case "uv":
      case "python":
      case "venv":
      case "dependencies":
      case "extractscripts":
      case "videos":
        return () => <LoadStep action={action} percent={percent} />;
      case "finish":
        return () => (
          <FinalStep
            folder={overrideFolder ?? undefined}
            onFinish={() => {
              void invalidatePostSetupState();
              setSetupProgress(defaultProgress);
              setShouldRunSetup(false);
            }}
          />
        );
      case "error":
        return () => <ErrorStep error={action} />;
    }
  }, [step, action, percent, overrideFolder, invalidatePostSetupState, setShouldRunSetup]);

  return (
    <AlertDialog open={shouldRunSetup}>
      <AlertDialogContent data-nav-passthrough onEscapeKeyDown={(e) => e.preventDefault()}>
        <img src={logoSrc} width={80} height={80} />
        <Step />
      </AlertDialogContent>
    </AlertDialog>
  );
};
