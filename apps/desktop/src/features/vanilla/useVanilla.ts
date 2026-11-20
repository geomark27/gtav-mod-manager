import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { VanillaStatusDto } from "../../bindings/VanillaStatusDto";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** Vanilla mode of an installation: restore point and mods still in the game. */
export function useVanillaStatus(installationId: string) {
  return useQuery<VanillaStatusDto, AppError>({
    queryKey: ["vanilla", installationId],
    queryFn: () => commands.vanillaStatus(installationId),
    retry: false,
  });
}

/** Saves a restore point and disables every mod; pending until the refreshed data is loaded. */
export function useEnterVanilla(installationId: string) {
  const client = useQueryClient();
  return useMutation<VanillaStatusDto, AppError, void>({
    mutationFn: () => commands.enterVanilla(installationId),
    onSuccess: async (status) => {
      client.setQueryData(["vanilla", installationId], status);
      await invalidateModData(client, installationId);
    },
  });
}

/** Applies the restore point; pending until the refreshed data is loaded. */
export function useRestoreFromVanilla(installationId: string) {
  const client = useQueryClient();
  return useMutation<VanillaStatusDto, AppError, void>({
    mutationFn: () => commands.restoreFromVanilla(installationId),
    onSuccess: async (status) => {
      client.setQueryData(["vanilla", installationId], status);
      await invalidateModData(client, installationId);
    },
  });
}
