import type { InventoryItemDto } from "../../bindings/InventoryItemDto";
import type { InventorySummaryDto } from "../../bindings/InventorySummaryDto";
import type { ProgressDto } from "../../bindings/ProgressDto";

/** Builds an inventory item for tests. */
export function makeItem(over: Partial<InventoryItemDto> = {}): InventoryItemDto {
  return {
    relPath: "GTA5.exe",
    size: 2048,
    mtimeNs: "1700000000000000000",
    sha256: null,
    classification: "original",
    suggestedCategory: null,
    evidence: [{ kind: "catalog_original" }],
    firstSeenAt: "2026-01-05T15:04:00Z",
    lastSeenAt: "2026-01-05T15:04:00Z",
    ...over,
  };
}

/** Builds a summary for tests. */
export function makeSummary(over: Partial<InventorySummaryDto> = {}): InventorySummaryDto {
  return {
    counts: [],
    lastScan: null,
    baseline: { entries: 0, trust: "none", capturedAt: null },
    ...over,
  };
}

/** Builds a progress event for tests. */
export function makeProgress(over: Partial<ProgressDto> = {}): ProgressDto {
  return {
    operationId: "op-1",
    kind: "scan",
    filesSeen: 0,
    bytesHashed: 0,
    current: null,
    done: false,
    ...over,
  };
}

/** A promise that can be resolved or rejected from the outside. */
export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
