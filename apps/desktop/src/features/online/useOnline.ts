import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { OnlineCheckDto } from "../../bindings/OnlineCheckDto";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** Scans and lists what is left before playing online (never changes the game folder). */
export function useCheckOnline(installationId: string) {
  const client = useQueryClient();
  return useMutation<OnlineCheckDto, AppError, void>({
    mutationFn: () => commands.checkOnlineReadiness(installationId),
    // The scan refreshed the inventory and the drift flags.
    onSuccess: () => invalidateModData(client, installationId),
  });
}

/** Scans, disables every mod (vanilla mode) and runs the checks. */
export function usePrepareOnline(installationId: string) {
  const client = useQueryClient();
  return useMutation<OnlineCheckDto, AppError, void>({
    mutationFn: () => commands.prepareForOnline(installationId),
    onSuccess: () => invalidateModData(client, installationId),
  });
}
