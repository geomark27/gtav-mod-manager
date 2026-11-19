import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import type { OperationKind } from "../bindings/OperationKind";
import type { ProgressDto } from "../bindings/ProgressDto";

/** Name of the backend event that carries `ProgressDto`. */
export const PROGRESS_EVENT = "gmm://progress";

/**
 * Latest progress event of the given operation kind, or `null` before the first one.
 * Subscribes on mount and unsubscribes on unmount.
 */
export function useProgress(kind: OperationKind): ProgressDto | null {
  const [progress, setProgress] = useState<ProgressDto | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    listen<ProgressDto>(PROGRESS_EVENT, (event) => {
      if (event.payload.kind === kind) setProgress(event.payload);
    })
      .then((fn) => {
        if (active) unlisten = fn;
        else fn();
      })
      .catch(() => {
        // Without events the operation still completes; only live progress is lost.
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [kind]);

  return progress;
}
