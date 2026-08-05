/**
 * Live party queue: seeds from `party_queue_list`, then stays current by
 * subscribing to `party.queue` broadcasts. Every guest and admin page uses this
 * so they all see the same ordered queue.
 */

import { useEffect, useState } from "react";
import { listen } from "@/bridge/runtime";
import { partyQueueList } from "@/bridge/party";
import type { PartyQueue } from "@/types/party";

export const usePartyQueue = (): PartyQueue => {
  const [queue, setQueue] = useState<PartyQueue>({ entries: [] });

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    // Subscribe before the initial fetch so a broadcast between them is not
    // lost (the WS does not replay). If the socket delivers a fresher snapshot
    // first, the initial fetch may momentarily overwrite it, but the next
    // broadcast reconciles.
    listen<PartyQueue>("party.queue", ({ payload }) => {
      if (!cancelled) setQueue(payload);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    partyQueueList()
      .then((initial) => {
        if (!cancelled) setQueue(initial);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return queue;
};
