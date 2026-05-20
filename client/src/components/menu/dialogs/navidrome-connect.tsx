import type { UseMutationResult } from "@tanstack/react-query";
import { CheckCircle2Icon, Loader2Icon, XCircleIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useDialog } from "@/hooks/use-dialog";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { useConnectNavidrome, useNavidromeLogin } from "@/mutations/use-source-mutations";
import { cn } from "@/lib/utils";

const normaliseUrl = (raw: string) => raw.trim().replace(/\/+$/, "");

type Form = {
  baseUrl: string;
  username: string;
  password: string;
};

const EMPTY_FORM: Form = { baseUrl: "", username: "", password: "" };

export const NavidromeConnectDialog = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const { mode, close } = useDialog();
  const open = mode === "navidrome-connect";

  const [form, setForm] = useState<Form>(EMPTY_FORM);

  const testMutation = useNavidromeLogin();
  const connectMutation = useConnectNavidrome();

  // Editing any field resets the test pill back to idle so the user doesn't
  // get a stale green check on credentials that no longer match what they
  // typed.
  const updateField =
    <K extends keyof Form>(key: K) =>
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setForm((prev) => ({ ...prev, [key]: e.target.value }));
      if (testMutation.status !== "idle") {
        testMutation.reset();
      }
    };

  useEffect(() => {
    if (!open) {
      setForm(EMPTY_FORM);
      testMutation.reset();
      connectMutation.reset();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const { focusedIndex } = useDialogNav({
    open,
    itemCount: 3,
    onBack: close,
    containerRef,
  });

  const canSubmit =
    form.baseUrl.trim().length > 0 && form.username.trim().length > 0 && form.password.length > 0;

  const isBusy = testMutation.isPending || connectMutation.isPending;

  type SubmitConfig<TData> = {
    mutation: UseMutationResult<TData, Error, Form>;
    onSuccess?: (data: TData) => void;
    onError?: (error: Error) => void;
  };

  const submit =
    <TData,>({ mutation, onSuccess, onError }: SubmitConfig<TData>) =>
    () => {
      if (!canSubmit || isBusy) return;
      mutation.mutate(
        {
          baseUrl: normaliseUrl(form.baseUrl),
          username: form.username.trim(),
          password: form.password,
        },
        { onSuccess, onError },
      );
    };

  const handleTest = submit({
    mutation: testMutation,
    onError: (e) => toast.error(`Could not reach server: ${e.message}`),
  });

  const handleConnect = submit({
    mutation: connectMutation,
    onSuccess: ({ login }) => {
      toast.success(`Library now reads from ${login.server_name ?? login.server_url}`);
      close();
    },
    onError: (e) => toast.error(`Login failed: ${e.message}`),
  });

  const reachedHost = testMutation.data?.server_name ?? testMutation.data?.server_url;

  const testState: {
    icon: React.ReactNode;
    tooltip: string;
  } = (() => {
    if (testMutation.isPending) {
      return {
        icon: <Loader2Icon className="size-4 animate-spin" />,
        tooltip: "Testing connection…",
      };
    }
    if (testMutation.isError) {
      return {
        icon: <XCircleIcon className="size-4 text-destructive" />,
        tooltip: `Could not reach server: ${testMutation.error.message}`,
      };
    }
    if (testMutation.isSuccess && reachedHost) {
      return {
        icon: <CheckCircle2Icon className="size-4 text-chart-3" />,
        tooltip: `Reached: ${reachedHost}`,
      };
    }
    return { icon: null, tooltip: "Test connection" };
  })();

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-md">
        <div className="contents">
          <DialogHeader>
            <DialogTitle>Connect to Navidrome</DialogTitle>
            <DialogDescription>
              Point Nightingale at a Navidrome (Subsonic-compatible) server. Audio is downloaded to
              your local cache on first analysis so the rest of the karaoke pipeline keeps working
              exactly like a folder library.
            </DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field>
              <Label htmlFor="navi-url">Server URL</Label>
              <Input
                id="navi-url"
                placeholder="https://navidrome.example.com"
                value={form.baseUrl}
                onChange={updateField("baseUrl")}
                disabled={isBusy}
              />
            </Field>
            <Field>
              <Label htmlFor="navi-user">Username</Label>
              <Input
                id="navi-user"
                autoComplete="username"
                value={form.username}
                onChange={updateField("username")}
                disabled={isBusy}
              />
            </Field>
            <Field>
              <Label htmlFor="navi-pass">Password</Label>
              <Input
                id="navi-pass"
                type="password"
                autoComplete="current-password"
                value={form.password}
                onChange={updateField("password")}
                disabled={isBusy}
              />
            </Field>
          </FieldGroup>
          <DialogFooter>
            <div
              ref={containerRef}
              className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end"
            >
              <DialogClose asChild>
                <Button
                  variant="outline"
                  onClick={close}
                  disabled={isBusy}
                  className={cn(
                    "focus-visible:ring-0 focus-visible:border-transparent",
                    focusedIndex === 0 && "ring-2 ring-primary",
                  )}
                >
                  Cancel
                </Button>
              </DialogClose>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    variant="outline"
                    disabled={!canSubmit || isBusy}
                    onClick={handleTest}
                    aria-label={testState.tooltip}
                    className={cn(
                      "focus-visible:ring-0 focus-visible:border-transparent",
                      focusedIndex === 1 && "ring-2 ring-primary",
                    )}
                  >
                    {testState.icon}
                    Test connection
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{testState.tooltip}</TooltipContent>
              </Tooltip>
              <Tooltip>
                <TooltipTrigger asChild>
                  {/* Wrapping span so the tooltip still fires while the
                      Button is disabled (Radix triggers swallow events on
                      disabled buttons). */}
                  <span>
                    <Button
                      disabled={!canSubmit || isBusy || !testMutation.isSuccess}
                      onClick={handleConnect}
                      className={cn(
                        "focus-visible:ring-0 focus-visible:border-transparent",
                        focusedIndex === 2 && "ring-2 ring-primary",
                      )}
                    >
                      {connectMutation.isPending ? "Connecting…" : "Connect"}
                    </Button>
                  </span>
                </TooltipTrigger>
                <TooltipContent>
                  {testMutation.isSuccess
                    ? "Save these credentials as your library source"
                    : "Run a successful test first"}
                </TooltipContent>
              </Tooltip>
            </div>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
};
