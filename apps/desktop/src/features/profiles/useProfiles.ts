import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { ModDto } from "../../bindings/ModDto";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import type { ProfileDetailDto } from "../../bindings/ProfileDetailDto";
import type { ProfileDto } from "../../bindings/ProfileDto";
import type { ProfileEntryInput } from "../../bindings/ProfileEntryInput";
import { commands } from "../../lib/ipc";
import { invalidateModData } from "../mods/useMods";

/** Profiles of an installation (Vanilla first). */
export function useProfiles(installationId: string) {
  return useQuery<ProfileDto[], AppError>({
    queryKey: ["profiles", installationId],
    queryFn: () => commands.listProfiles(installationId),
    retry: false,
  });
}

/** Names offered when saving a profile. */
export function useProfileNameSuggestions() {
  return useQuery<string[], AppError>({
    queryKey: ["profile-name-suggestions"],
    queryFn: commands.profileNameSuggestions,
    retry: false,
  });
}

/** Entries of a stored profile. */
export function useProfileDetail(profileId: string) {
  return useQuery<ProfileDetailDto, AppError>({
    queryKey: ["profile", profileId],
    queryFn: () => commands.getProfile(profileId),
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });
}

/** Saves the current state of the mods as a new profile. */
export function useCreateProfile(installationId: string) {
  const client = useQueryClient();
  return useMutation<ProfileDto, AppError, string>({
    mutationFn: (name) => commands.createProfileFromCurrent(installationId, name),
    onSuccess: () => client.invalidateQueries({ queryKey: ["profiles", installationId] }),
  });
}

/** Replaces the entries of a profile. */
export function useUpdateProfileEntries(installationId: string, profileId: string) {
  const client = useQueryClient();
  return useMutation<ProfileDetailDto, AppError, ProfileEntryInput[]>({
    mutationFn: (entries) => commands.updateProfileEntries(profileId, entries),
    onSuccess: async () => {
      await Promise.all([
        client.invalidateQueries({ queryKey: ["profiles", installationId] }),
        client.invalidateQueries({ queryKey: ["profile", profileId] }),
      ]);
    },
  });
}

/** Renames a profile. */
export function useRenameProfile(installationId: string) {
  const client = useQueryClient();
  return useMutation<ProfileDto, AppError, { profileId: string; name: string }>({
    mutationFn: ({ profileId, name }) => commands.renameProfile(profileId, name),
    onSuccess: () => client.invalidateQueries({ queryKey: ["profiles", installationId] }),
  });
}

/** Deletes a profile. */
export function useDeleteProfile(installationId: string) {
  const client = useQueryClient();
  return useMutation<void, AppError, string>({
    mutationFn: (profileId) => commands.deleteProfile(profileId),
    onSuccess: () => client.invalidateQueries({ queryKey: ["profiles", installationId] }),
  });
}

/** Dry run of applying a profile. */
export function usePreviewApplyProfile(installationId: string, profileId: string) {
  return useQuery<PlanPreviewDto, AppError>({
    queryKey: ["profile-preview", installationId, profileId],
    queryFn: () => commands.previewApplyProfile(installationId, profileId),
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });
}

/** Applies a profile; stays pending until the refreshed data is loaded. */
export function useApplyProfile(installationId: string) {
  const client = useQueryClient();
  return useMutation<ModDto[], AppError, string>({
    mutationFn: (profileId) => commands.applyProfile(installationId, profileId),
    onSuccess: () => invalidateModData(client, installationId),
  });
}
