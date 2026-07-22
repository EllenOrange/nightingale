import { Loader2Icon } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { openUrl } from "@/bridge/opener";
import { plexBeginPin, plexManualLogin, plexPollPin } from "@/bridge/source";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldGroup } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useDialog } from "@/hooks/use-dialog";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { cn } from "@/lib/utils";
import { useConnectPlex } from "@/mutations/use-source-mutations";
import type { PlexPinStart } from "@/types/PlexPinStart";
import type { PlexServer } from "@/types/PlexServer";

export const PlexConnectDialog = () => {
  const containerRef = useRef<HTMLDivElement>(null);
  const { mode, close } = useDialog();
  const open = mode === "plex-connect";
  const [advanced, setAdvanced] = useState(false);
  const [pin, setPin] = useState<PlexPinStart | null>(null);
  const [servers, setServers] = useState<PlexServer[]>([]);
  const [serverIndex, setServerIndex] = useState(0);
  const [sectionIds, setSectionIds] = useState<string[]>([]);
  const [manualUrl, setManualUrl] = useState("");
  const [manualToken, setManualToken] = useState("");
  const [busy, setBusy] = useState(false);
  const polling = useRef(false);
  const connect = useConnectPlex();

  const server = servers[serverIndex];
  const sections = server?.sections ?? [];
  const canConnect = !!server && sectionIds.length > 0 && !connect.isPending;
  const { focusedIndex } = useDialogNav({
    open,
    itemCount: servers.length > 0 ? 2 : 3,
    onBack: close,
    containerRef,
  });
  const navClass = (index: number) =>
    cn(
      "focus-visible:ring-0 focus-visible:border-transparent",
      focusedIndex === index && "ring-2 ring-primary",
    );

  useEffect(() => {
    if (open) return;
    setAdvanced(false);
    setPin(null);
    setServers([]);
    setServerIndex(0);
    setSectionIds([]);
    setManualUrl("");
    setManualToken("");
    setBusy(false);
    connect.reset();
    // mutation refs are stable across renders
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    setSectionIds([]);
  }, [serverIndex]);

  useEffect(() => {
    if (!open || !pin || servers.length > 0) return;
    let cancelled = false;

    const poll = async () => {
      if (polling.current) return;
      polling.current = true;
      try {
        const result = await plexPollPin({ pinId: pin.pin_id, clientId: pin.client_id });
        if (cancelled || !result.authorized) return;
        setPin(null);
        setServers(result.servers);
        setServerIndex(0);
        if (result.servers.length === 0) {
          toast.error("Plex sign-in succeeded, but no reachable media servers were discovered.");
        }
      } catch (error) {
        if (!cancelled) {
          toast.error(`Plex sign-in failed: ${(error as Error).message}`);
          setPin(null);
        }
      } finally {
        polling.current = false;
      }
    };

    void poll();
    const timer = window.setInterval(() => void poll(), 2_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [open, pin, servers.length]);

  const selectedSummary = useMemo(
    () =>
      sections.filter((section) => sectionIds.includes(section.id)).map((section) => section.title),
    [sectionIds, sections],
  );

  const startHostedSignIn = async () => {
    setBusy(true);
    try {
      const nextPin = await plexBeginPin();
      setPin(nextPin);
      await openUrl(nextPin.auth_url);
    } catch (error) {
      toast.error(`Could not start Plex sign-in: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const verifyManualServer = async () => {
    if (!manualUrl.trim() || !manualToken) return;
    setBusy(true);
    try {
      const discovered = await plexManualLogin({
        baseUrl: manualUrl.trim().replace(/\/+$/, ""),
        accessToken: manualToken,
      });
      setServers([discovered]);
      setServerIndex(0);
      setManualToken("");
    } catch (error) {
      toast.error(`Could not reach Plex: ${(error as Error).message}`);
    } finally {
      setBusy(false);
    }
  };

  const handleConnect = () => {
    if (!server || sectionIds.length === 0) return;
    connect.mutate(
      { server, sectionIds },
      {
        onSuccess: () => {
          toast.success(
            `Library now reads ${selectedSummary.join(", ")} from ${server.server_name}`,
          );
          close();
        },
        onError: (error) => toast.error(`Could not connect Plex: ${error.message}`),
      },
    );
  };

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Connect to Plex</DialogTitle>
          <DialogDescription>
            Connect through Plex on the web, or use a server URL and API token.
          </DialogDescription>
        </DialogHeader>

        {servers.length === 0 && !advanced && pin && (
          <p className="-mt-2 flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2Icon className="size-3.5 animate-spin" />
            Waiting for Plex authorization (code {pin.code})…
          </p>
        )}

        {servers.length === 0 && advanced && (
          <FieldGroup>
            <Field>
              <Label htmlFor="plex-url">Plex Media Server URL</Label>
              <Input
                id="plex-url"
                placeholder="https://plex.example.com:32400"
                value={manualUrl}
                onChange={(event) => setManualUrl(event.target.value)}
                disabled={busy}
              />
            </Field>
            <Field>
              <Label htmlFor="plex-token">Plex token</Label>
              <Input
                id="plex-token"
                type="password"
                autoComplete="off"
                value={manualToken}
                onChange={(event) => setManualToken(event.target.value)}
                disabled={busy}
              />
            </Field>
          </FieldGroup>
        )}

        {servers.length > 0 && (
          <div className="space-y-4">
            <Field>
              <Label htmlFor="plex-server">Server</Label>
              <Select
                value={String(serverIndex)}
                onValueChange={(value) => setServerIndex(Number(value))}
              >
                <SelectTrigger id="plex-server" className="w-full">
                  <SelectValue placeholder="Select a server" />
                </SelectTrigger>
                <SelectContent position="popper" className="w-[var(--radix-select-trigger-width)]">
                  {servers.map((candidate, index) => (
                    <SelectItem
                      key={`${candidate.server_id}:${candidate.server_url}`}
                      value={String(index)}
                    >
                      {candidate.server_name}
                      {candidate.owned ? "" : " (shared)"}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>

            <div className="space-y-2">
              <Label>Music libraries</Label>
              {sections.length === 0 ? (
                <p className="text-sm text-destructive">
                  This server did not expose any music libraries. Movie and TV libraries are not
                  imported.
                </p>
              ) : (
                sections.map((section) => (
                  <label key={section.id} className="flex items-center gap-2 text-sm">
                    <Checkbox
                      checked={sectionIds.includes(section.id)}
                      onCheckedChange={(checked) =>
                        setSectionIds((current) =>
                          checked
                            ? [...current, section.id]
                            : current.filter((id) => id !== section.id),
                        )
                      }
                    />
                    {section.title}
                  </label>
                ))
              )}
            </div>
          </div>
        )}

        <DialogFooter>
          <div
            ref={containerRef}
            className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end"
          >
            <Button
              variant="outline"
              onClick={close}
              disabled={busy || connect.isPending}
              className={navClass(0)}
            >
              Cancel
            </Button>
            {servers.length === 0 && !advanced && (
              <>
                <Button
                  variant="outline"
                  onClick={() => setAdvanced(true)}
                  disabled={busy}
                  className={navClass(1)}
                >
                  Advanced
                </Button>
                <Button
                  onClick={pin ? () => openUrl(pin.auth_url) : startHostedSignIn}
                  disabled={busy}
                  className={navClass(2)}
                >
                  {busy && <Loader2Icon className="animate-spin" />}
                  {pin ? "Open Plex sign-in" : "Sign in with Plex"}
                </Button>
              </>
            )}
            {servers.length === 0 && advanced && (
              <>
                <Button
                  variant="outline"
                  onClick={() => setAdvanced(false)}
                  disabled={busy}
                  className={navClass(1)}
                >
                  Back
                </Button>
                <Button
                  onClick={verifyManualServer}
                  disabled={busy || !manualUrl.trim() || !manualToken}
                  className={navClass(2)}
                >
                  {busy && <Loader2Icon className="animate-spin" />}
                  Test server
                </Button>
              </>
            )}
            {servers.length > 0 && (
              <Button onClick={handleConnect} disabled={!canConnect} className={navClass(1)}>
                {connect.isPending && <Loader2Icon className="animate-spin" />}
                Connect selected libraries
              </Button>
            )}
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
};
