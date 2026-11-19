import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { AppError } from "../../bindings/AppError";
import type { VaultReportDto } from "../../bindings/VaultReportDto";
import { commands } from "../../lib/ipc";

/** Size and contents of the vault. */
export function useVaultReport() {
  return useQuery<VaultReportDto, AppError>({
    queryKey: ["vault-report"],
    queryFn: commands.vaultReport,
    gcTime: 0,
    staleTime: 0,
    retry: false,
  });
}

/** Deletes unreferenced vault objects; resolves with the bytes freed. */
export function useVaultGc() {
  const client = useQueryClient();
  return useMutation<number, AppError, string[]>({
    mutationFn: (shas) => commands.vaultGc(shas),
    onSuccess: () => client.invalidateQueries({ queryKey: ["vault-report"] }),
  });
}
