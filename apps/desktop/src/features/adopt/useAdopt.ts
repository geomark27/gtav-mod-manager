import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AdoptPreviewDto } from "../../bindings/AdoptPreviewDto";
import type { AdoptRequest } from "../../bindings/AdoptRequest";
import type { AppError } from "../../bindings/AppError";
import type { ModDto } from "../../bindings/ModDto";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** What adopting the selected files would do. */
export function useAdoptPreview(installationId: string, relPaths: string[]) {
  return useQuery<AdoptPreviewDto, AppError>({
    queryKey: ["adopt-preview", installationId, relPaths],
    queryFn: () => commands.previewAdopt(installationId, relPaths),
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });
}

/** Adopts the selection as a mod, then refreshes mods and inventory. */
export function useAdoptMod(installationId: string) {
  const client = useQueryClient();
  return useMutation<ModDto, AppError, AdoptRequest>({
    mutationFn: commands.adoptMod,
    onSuccess: () => invalidateModData(client, installationId),
  });
}
