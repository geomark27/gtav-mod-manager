// Test helper: stands in for `@tauri-apps/api/event`. Use in a test file with
// `vi.mock("@tauri-apps/api/event", () => import("../../lib/eventMock"))`.
import type { ProgressDto } from "../bindings/ProgressDto";

type Handler = (event: { payload: ProgressDto }) => void;

const handlers = new Set<Handler>();

/** Fake `listen`: registers the handler and returns an unsubscribe function. */
export async function listen(_name: string, handler: Handler): Promise<() => void> {
  handlers.add(handler);
  return () => {
    handlers.delete(handler);
  };
}

/** Delivers a progress event to every subscriber. */
export function emitProgress(payload: ProgressDto): void {
  for (const h of [...handlers]) h({ payload });
}

/** Number of active subscriptions. */
export function listenerCount(): number {
  return handlers.size;
}
