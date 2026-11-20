import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { AppError } from "../../bindings/AppError";
import type { InventoryFilterDto } from "../../bindings/InventoryFilterDto";
import type { InventoryPageDto } from "../../bindings/InventoryPageDto";
import type { InventorySummaryDto } from "../../bindings/InventorySummaryDto";
import type { ScanSummaryDto } from "../../bindings/ScanSummaryDto";
import { commands } from "../../lib/ipc";

/** One page of the inventory; the previous page stays visible while the next loads. */
export function useInventory(installationId: string, filter: InventoryFilterDto) {
  return useQuery<InventoryPageDto, AppError>({
    queryKey: ["inventory", installationId, filter],
    queryFn: () => commands.listInventory(installationId, filter),
    placeholderData: keepPreviousData,
    retry: false,
  });
}

/** Counts per classification, last scan and baseline. */
export function useInventorySummary(installationId: string) {
  return useQuery<InventorySummaryDto, AppError>({
    queryKey: ["inventory-summary", installationId],
    queryFn: () => commands.inventorySummary(installationId),
    retry: false,
  });
}

/** Scans the installation, then refreshes everything derived from it. */
export function useScan(installationId: string) {
  const client = useQueryClient();
  return useMutation<ScanSummaryDto, AppError>({
    mutationFn: () => commands.scanInstallation(installationId),
    onSettled: async () => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["inventory", installationId] }),
        client.invalidateQueries({ queryKey: ["inventory-summary", installationId] }),
        client.invalidateQueries({ queryKey: ["detected", installationId] }),
        client.invalidateQueries({ queryKey: ["vanilla", installationId] }),
        // A scan re-validates the folder and may change the installation status.
        client.invalidateQueries({ queryKey: ["installations"] }),
      ]);
    },
  });
}

/** Asks the backend to stop the running operation. */
export function useCancelOperation() {
  return useMutation<void, AppError>({ mutationFn: commands.cancelOperation });
}

/** Returns `value` after it stopped changing for `delayMs`. */
export function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delayMs);
    return () => clearTimeout(timer);
  }, [value, delayMs]);
  return debounced;
}
