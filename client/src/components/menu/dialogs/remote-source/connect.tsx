import type { UseMutationResult } from "@tanstack/react-query";
import { CheckCircle2Icon, Loader2Icon, XCircleIcon } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent, type ReactNode } from "react";
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
import { cn } from "@/lib/utils";

const normaliseUrl = (raw: string) => raw.trim().replace(/\/+$/, "");

type RemoteSourceForm = {
  baseUrl: string;
  username: string;
  password: string;
};

const EMPTY_FORM: RemoteSourceForm = { baseUrl: "", username: "", password: "" };

type RemoteLoginResult = {
  server_name?: string | null;
  server_url: string;
};

type RemoteSourceConnectDialogProps<TLogin extends RemoteLoginResult> = {
  mode: "jellyfin-connect" | "navidrome-connect";
  title: string;
  description: ReactNode;
  urlInputId: string;
  urlPlaceholder: string;
  usernameInputId: string;
  passwordInputId: string;
  useLogin: () => UseMutationResult<TLogin, Error, RemoteSourceForm>;
  useConnect: () => UseMutationResult<{ login: TLogin }, Error, RemoteSourceForm>;
};

export const RemoteSourceConnectDialog = <TLogin extends RemoteLoginResult>({
  mode: dialogMode,
  title,
  description,
  urlInputId,
  urlPlaceholder,
  usernameInputId,
  passwordInputId,
  useLogin,
  useConnect,
}: RemoteSourceConnectDialogProps<TLogin>) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const { mode, close } = useDialog();
  const open = mode === dialogMode;

  const [form, setForm] = useState<RemoteSourceForm>(EMPTY_FORM);

  const testMutation = useLogin();
  const connectMutation = useConnect();

  // Editing any field resets the test pill back to idle so the user doesn't
  // get a stale green check on credentials that no longer match what they
  // typed.
  const updateField =
    <K extends keyof RemoteSourceForm>(key: K) =>
    (e: ChangeEvent<HTMLInputElement>) => {
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
    // mutation refs are stable across renders
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

  const submit =
    <TData,>({
      mutation,
      onSuccess,
      onError,
    }: {
      mutation: UseMutationResult<TData, Error, RemoteSourceForm>;
      onSuccess?: (data: TData) => void;
      onError?: (error: Error) => void;
    }) =>
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
    icon: ReactNode;
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
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>{description}</DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field>
              <Label htmlFor={urlInputId}>Server URL</Label>
              <Input
                id={urlInputId}
                placeholder={urlPlaceholder}
                value={form.baseUrl}
                onChange={updateField("baseUrl")}
                disabled={isBusy}
              />
            </Field>
            <Field>
              <Label htmlFor={usernameInputId}>Username</Label>
              <Input
                id={usernameInputId}
                autoComplete="username"
                value={form.username}
                onChange={updateField("username")}
                disabled={isBusy}
              />
            </Field>
            <Field>
              <Label htmlFor={passwordInputId}>Password</Label>
              <Input
                id={passwordInputId}
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
              <Button
                disabled={!canSubmit || isBusy || !testMutation.isSuccess}
                onClick={handleConnect}
                className={cn(
                  "focus-visible:ring-0 focus-visible:border-transparent",
                  focusedIndex === 2 && "ring-2 ring-primary",
                )}
              >
                Connect
              </Button>
            </div>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
};
