import { listen } from "@tauri-apps/api/event";
import type { QueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

/** Backend event sent after the tray changed data (prepare for online, restore my mods). */
export const DATA_CHANGED_EVENT = "gmm://data-changed";

/** Refreshes every query when the tray changed something behind the window's back. */
export function useDataChangedRefresh(client: QueryClient): void {
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listen(DATA_CHANGED_EVENT, () => {
      void client.invalidateQueries();
    })
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      })
      .catch(() => {
        // Without events the window refreshes on its next query; nothing is lost.
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [client]);
}
