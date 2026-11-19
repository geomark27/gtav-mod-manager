import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { DiscoveredInstallationDto } from "../../bindings/DiscoveredInstallationDto";
import type { InstallationDto } from "../../bindings/InstallationDto";
import type { RegisterInstallationRequest } from "../../bindings/RegisterInstallationRequest";
import { commands } from "../../lib/ipc";

export const INSTALLATIONS_KEY = ["installations"] as const;
export const SELECTED_KEY = ["installations", "selected"] as const;

/** Registered installations. */
export function useInstallations() {
  return useQuery<InstallationDto[], AppError>({
    queryKey: INSTALLATIONS_KEY,
    queryFn: commands.listInstallations,
    retry: false,
  });
}

/** The active installation, or `null`. */
export function useSelectedInstallation() {
  return useQuery<InstallationDto | null, AppError>({
    queryKey: SELECTED_KEY,
    queryFn: commands.getSelectedInstallation,
    retry: false,
  });
}

/** Runs detection; the result is held by the mutation, not cached. */
export function useDiscoverInstallations() {
  return useMutation<DiscoveredInstallationDto[], AppError>({
    mutationFn: commands.discoverInstallations,
  });
}

function useInvalidating() {
  const client = useQueryClient();
  // The prefix ["installations"] also covers the selected-installation query.
  return () => client.invalidateQueries({ queryKey: INSTALLATIONS_KEY });
}

/** Registers a folder. */
export function useRegisterInstallation() {
  const invalidate = useInvalidating();
  return useMutation<InstallationDto, AppError, RegisterInstallationRequest>({
    mutationFn: commands.registerInstallation,
    onSuccess: invalidate,
  });
}

/** Re-validates an installation. */
export function useRefreshInstallation() {
  const invalidate = useInvalidating();
  return useMutation<InstallationDto, AppError, string>({
    mutationFn: commands.refreshInstallation,
    onSuccess: invalidate,
  });
}

/** Renames an installation. */
export function useRenameInstallation() {
  const invalidate = useInvalidating();
  return useMutation<InstallationDto, AppError, { id: string; displayName: string }>({
    mutationFn: ({ id, displayName }) => commands.renameInstallation(id, displayName),
    onSuccess: invalidate,
  });
}

/** Selects the active installation. */
export function useSelectInstallation() {
  const invalidate = useInvalidating();
  return useMutation<void, AppError, string>({
    mutationFn: commands.selectInstallation,
    onSuccess: invalidate,
  });
}
