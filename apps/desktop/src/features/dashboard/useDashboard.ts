import { useQuery } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { DashboardDto } from "../../bindings/DashboardDto";
import { commands } from "../../lib/ipc";

/** Everything the dashboard shows for one installation. */
export function useDashboard(installationId: string) {
  return useQuery<DashboardDto, AppError>({
    queryKey: ["dashboard", installationId],
    queryFn: () => commands.dashboard(installationId),
    retry: false,
  });
}
