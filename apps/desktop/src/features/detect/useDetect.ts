import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AdoptDetectedRequest } from "../../bindings/AdoptDetectedRequest";
import type { AdoptDetectedResultDto } from "../../bindings/AdoptDetectedResultDto";
import type { AppError } from "../../bindings/AppError";
import type { DetectedModsDto } from "../../bindings/DetectedModsDto";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** Mods proposed from the last scan. */
export function useDetectedMods(installationId: string) {
  return useQuery<DetectedModsDto, AppError>({
    queryKey: ["detected", installationId],
    queryFn: () => commands.detectMods(installationId),
    retry: false,
  });
}

/** Adopts the selected proposals; pending until the refreshed data is loaded. */
export function useAdoptDetected(installationId: string) {
  const client = useQueryClient();
  return useMutation<AdoptDetectedResultDto, AppError, Omit<AdoptDetectedRequest, "installationId">>({
    mutationFn: (req) => commands.adoptDetected({ installationId, ...req }),
    onSuccess: () => invalidateModData(client, installationId),
  });
}
