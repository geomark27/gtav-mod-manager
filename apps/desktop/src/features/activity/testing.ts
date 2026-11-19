import type { TransactionDto } from "../../bindings/TransactionDto";

/** Builds a transaction for tests. */
export function makeTxn(over: Partial<TransactionDto> = {}): TransactionDto {
  return {
    id: "t1",
    kind: "enable",
    status: "committed",
    createdAt: "2026-01-05T15:04:00Z",
    finishedAt: "2026-01-05T15:04:05Z",
    error: null,
    stepCount: 3,
    modNames: ["Alpha"],
    description: "Enabled 1 mod",
    ...over,
  };
}
