import { useMutation, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { BaselineDto } from "../../bindings/BaselineDto";
import { commands } from "../../lib/ipc";

/** Captures the baseline; the summary (which carries the baseline) is refreshed afterwards. */
export function useCaptureBaseline(installationId: string) {
  const client = useQueryClient();
  return useMutation<BaselineDto, AppError, boolean>({
    mutationFn: (trusted) => commands.captureBaseline(installationId, trusted),
    onSettled: async () => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["inventory-summary", installationId] }),
        client.invalidateQueries({ queryKey: ["installations"] }),
      ]);
    },
  });
}
