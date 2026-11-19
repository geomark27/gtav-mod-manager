import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { AppStatusDto } from "../../bindings/AppStatusDto";
import { commands } from "../../lib/ipc";

/** Loads the system status from the backend. */
export function useAppStatus() {
  return useQuery<AppStatusDto, AppError>({
    queryKey: ["app-status"],
    queryFn: commands.getAppStatus,
    retry: false,
  });
}
