import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { ConfirmImportRequest } from "../../bindings/ConfirmImportRequest";
import type { ImportPreviewDto } from "../../bindings/ImportPreviewDto";
import type { ModDto } from "../../bindings/ModDto";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** Reads a ZIP archive into the staging area (cancellable). */
export function useStageImport(installationId: string) {
  return useMutation<ImportPreviewDto, AppError, string>({
    mutationFn: (zipPath) => commands.stageImport(installationId, zipPath),
  });
}

/** Recomputes the mapping of a staged archive for another root folder. */
export function useRemapImport(stagingId: string) {
  return useMutation<ImportPreviewDto, AppError, string | null>({
    mutationFn: (root) => commands.remapImport(stagingId, root),
  });
}

/** Registers the staged archive as a mod. Refreshes mod data on success and on failure. */
export function useConfirmImport(installationId: string) {
  const client = useQueryClient();
  return useMutation<ModDto, AppError, ConfirmImportRequest>({
    mutationFn: (req) => commands.confirmImport(req),
    onSettled: () => invalidateModData(client, installationId),
  });
}

/** Throws away a staged archive. */
export function useDiscardImport() {
  return useMutation<void, AppError, string>({
    mutationFn: (stagingId) => commands.discardImport(stagingId),
  });
}
