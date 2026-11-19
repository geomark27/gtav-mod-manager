import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { commands } from "../../lib/ipc";

/** Delay before the first automatic check (10 s after start). */
export const AUTO_CHECK_DELAY_MS = 10_000;

/** Time between automatic checks (6 h). */
export const AUTO_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

/** How often the running-operation state is polled while the banner is visible. */
export const OPERATION_POLL_MS = 2_000;

/** Stage of the updater. */
export type UpdatePhase =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "error";

/** A newer version offered by the release endpoint. */
export interface AvailableUpdate {
  /** Version offered by the update server. */
  version: string;
  /** Release notes as plain text, when the server provided them. */
  notes: string | null;
}

interface UpdaterState {
  phase: UpdatePhase;
  available: AvailableUpdate | null;
  downloaded: number;
  total: number | null;
  /** Error of a manual check or of the install; automatic-check errors are only logged. */
  error: string | null;
  /** ISO timestamp of the last finished check, `null` before the first one. */
  lastCheckAt: string | null;
  /** True after **Later**, until the next check finds an update. */
  dismissed: boolean;
}

const INITIAL_STATE: UpdaterState = {
  phase: "idle",
  available: null,
  downloaded: 0,
  total: null,
  error: null,
  lastCheckAt: null,
  dismissed: false,
};

/** State and actions shared by the banner and the Settings card. */
export interface UpdaterApi extends UpdaterState {
  /** True while a backend operation holds the single operation slot. */
  operationBusy: boolean;
  /** Checks for an update and shows the result in the UI. */
  checkNow: () => Promise<void>;
  /** Downloads, installs and relaunches; refuses while an operation is running. */
  install: () => Promise<void>;
  /** Hides the banner until the next check. */
  dismiss: () => void;
}

const UpdatesContext = createContext<UpdaterApi | null>(null);

function describeError(value: unknown): string {
  if (typeof value === "string") return value;
  if (value instanceof Error) return value.message;
  return "The update could not be completed.";
}

/** Provides the shared updater state. Mount once, above the banner and the routes. */
export function UpdatesProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<UpdaterState>(INITIAL_STATE);
  const [operationBusy, setOperationBusy] = useState(false);
  const updateRef = useRef<Update | null>(null);

  const runCheck = useCallback(async (manual: boolean) => {
    setState((s) => ({ ...s, phase: "checking", error: manual ? null : s.error }));
    try {
      const found = await check();
      updateRef.current = found;
      setState((s) => ({
        ...s,
        phase: found ? "available" : "idle",
        available: found ? { version: found.version, notes: found.body ?? null } : null,
        downloaded: 0,
        total: null,
        error: null,
        lastCheckAt: new Date().toISOString(),
        dismissed: false,
      }));
    } catch (value) {
      if (manual) {
        setState((s) => ({
          ...s,
          phase: "error",
          error: describeError(value),
          lastCheckAt: new Date().toISOString(),
        }));
      } else {
        // Automatic checks never show a banner; they only go to the console.
        console.warn("Automatic update check failed:", value);
        setState((s) => ({ ...s, phase: "idle" }));
      }
    }
  }, []);

  const checkNow = useCallback(() => runCheck(true), [runCheck]);

  const install = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    try {
      const status = await commands.operationStatus();
      setOperationBusy(status.busy);
      if (status.busy) return;
    } catch (value) {
      // Without a confirmed idle slot, installing would risk interrupting an operation.
      setState((s) => ({ ...s, error: describeError(value) }));
      return;
    }
    setState((s) => ({
      ...s,
      phase: "downloading",
      downloaded: 0,
      total: null,
      error: null,
    }));
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          setState((s) => ({ ...s, total: event.data.contentLength ?? null }));
        } else if (event.event === "Progress") {
          setState((s) => ({ ...s, downloaded: s.downloaded + event.data.chunkLength }));
        }
      });
      setState((s) => ({ ...s, phase: "ready" }));
      await relaunch();
    } catch (value) {
      setState((s) => ({ ...s, phase: "error", error: describeError(value) }));
    }
  }, []);

  const dismiss = useCallback(() => {
    setState((s) => ({ ...s, dismissed: true }));
  }, []);

  // Automatic checks are off in development, where no signed release exists. Manual checks
  // still work.
  useEffect(() => {
    if (import.meta.env.DEV) return;
    const first = setTimeout(() => {
      void runCheck(false);
    }, AUTO_CHECK_DELAY_MS);
    const every = setInterval(() => {
      void runCheck(false);
    }, AUTO_CHECK_INTERVAL_MS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, [runCheck]);

  // Poll the operation slot while the banner is visible so "Update now" is blocked during
  // scans, transactions and imports.
  const visible = state.available !== null && !state.dismissed;
  useEffect(() => {
    if (!visible) {
      setOperationBusy(false);
      return;
    }
    let active = true;
    const poll = () => {
      commands
        .operationStatus()
        .then((status) => {
          if (active) setOperationBusy(status.busy);
        })
        .catch(() => {
          // A failed poll does not hide the banner; the next one may succeed.
        });
    };
    poll();
    const id = setInterval(poll, OPERATION_POLL_MS);
    return () => {
      active = false;
      clearInterval(id);
    };
  }, [visible]);

  const value: UpdaterApi = { ...state, operationBusy, checkNow, install, dismiss };
  return <UpdatesContext.Provider value={value}>{children}</UpdatesContext.Provider>;
}

/** Shared updater state. Must be used inside [`UpdatesProvider`]. */
export function useUpdater(): UpdaterApi {
  const value = useContext(UpdatesContext);
  if (!value) throw new Error("useUpdater must be used inside UpdatesProvider");
  return value;
}
