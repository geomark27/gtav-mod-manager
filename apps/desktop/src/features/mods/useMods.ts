import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { QueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { Category } from "../../bindings/Category";
import type { DriftDto } from "../../bindings/DriftDto";
import type { ModDetailDto } from "../../bindings/ModDetailDto";
import type { ModDto } from "../../bindings/ModDto";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import { commands } from "../../lib/ipc";

/** Refreshes everything derived from the mods of an installation. */
export function invalidateModData(client: QueryClient, installationId: string): Promise<unknown> {
  return Promise.all([
    client.invalidateQueries({ queryKey: ["mods", installationId] }),
    client.invalidateQueries({ queryKey: ["inventory", installationId] }),
    client.invalidateQueries({ queryKey: ["inventory-summary", installationId] }),
    client.invalidateQueries({ queryKey: ["dashboard", installationId] }),
    client.invalidateQueries({ queryKey: ["transactions", installationId] }),
    client.invalidateQueries({ queryKey: ["profiles", installationId] }),
    client.invalidateQueries({ queryKey: ["profile"] }),
    client.invalidateQueries({ queryKey: ["vanilla", installationId] }),
    client.invalidateQueries({ queryKey: ["detected", installationId] }),
  ]);
}

/** Puts server-returned mods into the cached list (state always comes from the server). */
function mergeReturned(client: QueryClient, installationId: string, returned: ModDto[]) {
  client.setQueryData<ModDto[]>(["mods", installationId], (current) =>
    current?.map((m) => returned.find((r) => r.id === m.id) ?? m),
  );
}

/** Managed mods of an installation. */
export function useMods(installationId: string, enabled: boolean) {
  return useQuery<ModDto[], AppError>({
    queryKey: ["mods", installationId],
    queryFn: () => commands.listMods(installationId),
    enabled,
    retry: false,
  });
}

/**
 * Compares the disk with the database. Runs when the page opens (there is no reconcile at
 * startup) and again on `refetch` ("Check again"); the mods list is refreshed afterwards.
 */
export function useReconcile(installationId: string) {
  const client = useQueryClient();
  return useQuery<DriftDto[], AppError>({
    queryKey: ["reconcile", installationId],
    queryFn: async () => {
      const drift = await commands.reconcileInstallation(installationId);
      await invalidateModData(client, installationId);
      return drift ?? [];
    },
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

/** Enables or disables one mod. Stays pending until the refreshed list is loaded. */
export function useSetModEnabled(installationId: string) {
  const client = useQueryClient();
  return useMutation<
    ModDto,
    AppError,
    { modId: string; enabled: boolean; withDependencies?: boolean }
  >({
    mutationFn: ({ modId, enabled, withDependencies }) =>
      commands.setModEnabled(modId, enabled, withDependencies ?? false),
    onSuccess: async (mod) => {
      mergeReturned(client, installationId, [mod]);
      await invalidateModData(client, installationId);
    },
  });
}

/** Dry run of a category switch. */
export function usePreviewCategory(
  installationId: string,
  category: Category,
  enabled: boolean,
  open: boolean,
) {
  return useQuery<PlanPreviewDto, AppError>({
    queryKey: ["category-preview", installationId, category, enabled],
    queryFn: () => commands.previewSetCategory(installationId, category, enabled),
    enabled: open,
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });
}

/** Enables or disables a whole category. */
export function useSetCategoryEnabled(installationId: string) {
  const client = useQueryClient();
  return useMutation<ModDto[], AppError, { category: Category; enabled: boolean }>({
    mutationFn: ({ category, enabled }) =>
      commands.setCategoryEnabled(installationId, category, enabled),
    onSuccess: async (mods) => {
      mergeReturned(client, installationId, mods);
      await invalidateModData(client, installationId);
    },
  });
}

/** One mod with its files. */
export function useModDetail(modId: string) {
  return useQuery<ModDetailDto, AppError>({
    queryKey: ["mod-detail", modId],
    queryFn: () => commands.getMod(modId),
    gcTime: 0,
    retry: false,
  });
}

/** Saves name, category and notes. */
export function useUpdateMod(installationId: string, modId: string) {
  const client = useQueryClient();
  return useMutation<ModDto, AppError, { name: string; category: Category; notes: string | null }>({
    mutationFn: ({ name, category, notes }) => commands.updateMod(modId, name, category, notes),
    onSuccess: async () => {
      await Promise.all([
        invalidateModData(client, installationId),
        client.invalidateQueries({ queryKey: ["mod-detail", modId] }),
      ]);
    },
  });
}

/** Stops managing a mod; its files stay in the game folder. */
export function useForgetMod(installationId: string) {
  const client = useQueryClient();
  return useMutation<void, AppError, string>({
    mutationFn: (modId) => commands.forgetMod(modId),
    onSuccess: () => invalidateModData(client, installationId),
  });
}

/** Puts the managed version of a drifted mod back; the current files go to the vault. */
export function useRestoreMod(installationId: string) {
  const client = useQueryClient();
  return useMutation<ModDto, AppError, string>({
    mutationFn: (modId) => commands.restoreMod(modId),
    onSuccess: async (mod) => {
      mergeReturned(client, installationId, [mod]);
      await invalidateModData(client, installationId);
    },
  });
}

/** Makes the current files of a drifted mod the managed version. */
export function useReadoptMod(installationId: string) {
  const client = useQueryClient();
  return useMutation<ModDto, AppError, string>({
    mutationFn: (modId) => commands.readoptMod(modId),
    onSuccess: async (mod) => {
      mergeReturned(client, installationId, [mod]);
      await invalidateModData(client, installationId);
    },
  });
}
