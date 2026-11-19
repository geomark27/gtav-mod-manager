import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { GuardStatusDto } from "../../bindings/GuardStatusDto";
import { commands } from "../../lib/ipc";

/** How often the guard state is refreshed, in milliseconds. */
export const GUARD_POLL_MS = 5000;

/** Guard state of an installation, polled every 5 seconds. */
export function useGuardStatus(installationId: string) {
  return useQuery<GuardStatusDto, AppError>({
    queryKey: ["guard", installationId],
    queryFn: () => commands.guardStatus(installationId),
    refetchInterval: GUARD_POLL_MS,
    retry: false,
  });
}
