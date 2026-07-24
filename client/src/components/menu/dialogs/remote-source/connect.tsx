import type { UseMutationResult } from "@tanstack/react-query";
import { CheckCircle2Icon, Loader2Icon, XCircleIcon } from "lucide-react";
import { useEffect, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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

type SelectableItem = { id: string; label: string };

type ConnectVariables = RemoteSourceForm & { selectedIds: string[] };

type RemoteSourceConnectDialogProps<TLogin extends RemoteLoginResult> = {
  mode: "jellyfin-connect" | "navidrome-connect";
  title: string;
  description: ReactNode;
  urlInputId: string;
  urlPlaceholder: string;
  usernameInputId: string;
  passwordInputId: string;
  useLogin: () => UseMutationResult<TLogin, Error, RemoteSourceForm>;
  useConnect: () => UseMutationResult<{ login: TLogin }, Error, ConnectVariables>;
  /**
   * Optional post-test selection step. When the test succeeds and this yields
   * items, the dialog shows a checkbox list and only enables Connect once at
   * least one item is picked. The chosen ids are forwarded to `useConnect`.
   */
  selection?: {
    label: string;
    emptyMessage: ReactNode;
    getItems: (login: TLogin) => SelectableItem[];
  };
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
  selection,
}: RemoteSourceConnectDialogProps<TLogin>) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const { mode, close } = useDialog();
  const open = mode === dialogMode;

  const [form, setForm] = useState<RemoteSourceForm>(EMPTY_FORM);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);

  const testMutation = useLogin();
  const connectMutation = useConnect();

  // Editing any field resets the test pill back to idle so the user doesn't
  // get a stale green check on credentials that no longer match what they
  // typed — and clears any selection made against the old server.
  const updateField =
    <K extends keyof RemoteSourceForm>(key: K) =>
    (e: ChangeEvent<HTMLInputElement>) => {
      setForm((prev) => ({ ...prev, [key]: e.target.value }));
      if (testMutation.status !== "idle") {
        testMutation.reset();
        setSelectedIds([]);
      }
    };

  useEffect(() => {
    if (!open) {
      setForm(EMPTY_FORM);
      setSelectedIds([]);
      testMutation.reset();
      connectMutation.reset();
    }
    // mutation refs are stable across renders
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const selectableItems =
    selection && testMutation.isSuccess ? selection.getItems(testMutation.data) : [];
  // Only force a choice when there's actually something to choose. A server
  // that exposes no libraries (or a failed listing) falls back to importing
  // everything.
  const selectionRequired = !!selection && selectableItems.length > 0;

  const { focusedIndex } = useDialogNav({
    open,
    itemCount: 3,
    onBack: close,
    containerRef,
  });

  const canSubmit =
    form.baseUrl.trim().length > 0 && form.username.trim().length > 0 && form.password.length > 0;

  const isBusy = testMutation.isPending || connectMutation.isPending;

  const credentials = () => ({
    baseUrl: normaliseUrl(form.baseUrl),
    username: form.username.trim(),
    password: form.password,
  });

  const handleTest = () => {
    if (!canSubmit || isBusy) return;
    testMutation.mutate(credentials(), {
      onError: (e) => toast.error(`Could not reach server: ${e.message}`),
    });
  };

  const canConnect =
    canSubmit &&
    !isBusy &&
    testMutation.isSuccess &&
    (!selectionRequired || selectedIds.length > 0);

  const handleConnect = () => {
    if (!canConnect) return;
    connectMutation.mutate(
      { ...credentials(), selectedIds },
      {
        onSuccess: ({ login }) => {
          toast.success(`Library now reads from ${login.server_name ?? login.server_url}`);
          close();
        },
        onError: (e) => toast.error(`Login failed: ${e.message}`),
      },
    );
  };

  const toggleSelected = (id: string, checked: boolean) =>
    setSelectedIds((current) =>
      checked ? [...current, id] : current.filter((existing) => existing !== id),
    );

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
          {selection && testMutation.isSuccess && (
            <div className="space-y-2">
              <Label>{selection.label}</Label>
              {selectableItems.length === 0 ? (
                <p className="text-sm text-muted-foreground">{selection.emptyMessage}</p>
              ) : (
                <div className="flex max-h-48 flex-wrap content-start gap-x-4 gap-y-2 overflow-y-auto overscroll-contain">
                  {selectableItems.map((item) => (
                    <label key={item.id} className="flex items-center gap-2 text-sm">
                      <Checkbox
                        className="after:inset-0"
                        checked={selectedIds.includes(item.id)}
                        onCheckedChange={(checked) => toggleSelected(item.id, checked === true)}
                        disabled={isBusy}
                      />
                      {item.label}
                    </label>
                  ))}
                </div>
              )}
            </div>
          )}
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
                disabled={!canConnect}
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
