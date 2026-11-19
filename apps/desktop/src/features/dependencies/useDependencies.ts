import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { ModDetailDto } from "../../bindings/ModDetailDto";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** Keeps the details cache and every derived list in step after an edge changed. */
function useAfterEdgeChange(installationId: string, modId: string) {
  const client = useQueryClient();
  return async (detail: ModDetailDto) => {
    client.setQueryData<ModDetailDto>(["mod-detail", modId], detail);
    await invalidateModData(client, installationId);
  };
}

/** Makes `modId` require another mod. */
export function useAddDependency(installationId: string, modId: string) {
  const after = useAfterEdgeChange(installationId, modId);
  return useMutation<ModDetailDto, AppError, string>({
    mutationFn: (dependsOnModId) => commands.addDependency(modId, dependsOnModId),
    onSuccess: after,
  });
}

/** Removes a requirement of `modId`. */
export function useRemoveDependency(installationId: string, modId: string) {
  const after = useAfterEdgeChange(installationId, modId);
  return useMutation<ModDetailDto, AppError, string>({
    mutationFn: (dependsOnModId) => commands.removeDependency(modId, dependsOnModId),
    onSuccess: after,
  });
}

/** Dry run of enabling/disabling a mod, including its dependency closure. */
export function usePreviewModToggle() {
  return useMutation<PlanPreviewDto, AppError, { modId: string; enabled: boolean }>({
    mutationFn: ({ modId, enabled }) => commands.previewSetModEnabled(modId, enabled),
  });
}
