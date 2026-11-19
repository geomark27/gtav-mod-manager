import { invoke } from "@tauri-apps/api/core";
import type { AdoptPreviewDto } from "../bindings/AdoptPreviewDto";
import type { AdoptRequest } from "../bindings/AdoptRequest";
import type { AppError } from "../bindings/AppError";
import type { AppStatusDto } from "../bindings/AppStatusDto";
import type { BaselineDto } from "../bindings/BaselineDto";
import type { Category } from "../bindings/Category";
import type { DashboardDto } from "../bindings/DashboardDto";
import type { ConfirmImportRequest } from "../bindings/ConfirmImportRequest";
import type { DiscoveredInstallationDto } from "../bindings/DiscoveredInstallationDto";
import type { DriftDto } from "../bindings/DriftDto";
import type { GuardStatusDto } from "../bindings/GuardStatusDto";
import type { ImportPreviewDto } from "../bindings/ImportPreviewDto";
import type { InstallationDto } from "../bindings/InstallationDto";
import type { InventoryFilterDto } from "../bindings/InventoryFilterDto";
import type { InventoryPageDto } from "../bindings/InventoryPageDto";
import type { InventorySummaryDto } from "../bindings/InventorySummaryDto";
import type { ModDetailDto } from "../bindings/ModDetailDto";
import type { ModDto } from "../bindings/ModDto";
import type { OperationStatusDto } from "../bindings/OperationStatusDto";
import type { PlanPreviewDto } from "../bindings/PlanPreviewDto";
import type { ProfileDetailDto } from "../bindings/ProfileDetailDto";
import type { ProfileDto } from "../bindings/ProfileDto";
import type { ProfileEntryInput } from "../bindings/ProfileEntryInput";
import type { RegisterInstallationRequest } from "../bindings/RegisterInstallationRequest";
import type { ScanSummaryDto } from "../bindings/ScanSummaryDto";
import type { TransactionPageDto } from "../bindings/TransactionPageDto";
import type { VaultReportDto } from "../bindings/VaultReportDto";

function isAppError(value: unknown): value is AppError {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return typeof v.code === "string" && typeof v.message === "string";
}

/** Normalizes any rejection into the generated `AppError` shape. */
export function toAppError(value: unknown): AppError {
  if (isAppError(value)) {
    return {
      code: value.code,
      message: value.message,
      hint: typeof value.hint === "string" ? value.hint : null,
      details: typeof value.details === "string" ? value.details : null,
    };
  }
  return { code: "INTERNAL", message: String(value), hint: null, details: null };
}

/** Calls a Tauri command; rejections are always `AppError`. */
export async function invokeCommand<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (value) {
    throw toAppError(value);
  }
}

/** Typed wrappers for every backend command. */
export const commands = {
  getAppStatus: () => invokeCommand<AppStatusDto>("get_app_status"),
  discoverInstallations: () =>
    invokeCommand<DiscoveredInstallationDto[]>("discover_installations"),
  registerInstallation: (req: RegisterInstallationRequest) =>
    invokeCommand<InstallationDto>("register_installation", { req }),
  listInstallations: () => invokeCommand<InstallationDto[]>("list_installations"),
  refreshInstallation: (id: string) => invokeCommand<InstallationDto>("refresh_installation", { id }),
  renameInstallation: (id: string, displayName: string) =>
    invokeCommand<InstallationDto>("rename_installation", { id, displayName }),
  selectInstallation: (id: string) => invokeCommand<void>("select_installation", { id }),
  getSelectedInstallation: () =>
    invokeCommand<InstallationDto | null>("get_selected_installation"),
  scanInstallation: (id: string) => invokeCommand<ScanSummaryDto>("scan_installation", { id }),
  listInventory: (id: string, filter: InventoryFilterDto) =>
    invokeCommand<InventoryPageDto>("list_inventory", { id, filter }),
  inventorySummary: (id: string) => invokeCommand<InventorySummaryDto>("inventory_summary", { id }),
  captureBaseline: (id: string, trusted: boolean) =>
    invokeCommand<BaselineDto>("capture_baseline", { id, trusted }),
  cancelOperation: () => invokeCommand<void>("cancel_operation"),
  /** Whether a heavy operation is running; the updater waits for it before restarting. */
  operationStatus: () => invokeCommand<OperationStatusDto>("operation_status"),
  guardStatus: (installationId: string) =>
    invokeCommand<GuardStatusDto>("guard_status", { installationId }),
  listMods: (installationId: string) => invokeCommand<ModDto[]>("list_mods", { installationId }),
  getMod: (modId: string) => invokeCommand<ModDetailDto>("get_mod", { modId }),
  previewAdopt: (installationId: string, relPaths: string[]) =>
    invokeCommand<AdoptPreviewDto>("preview_adopt", { installationId, relPaths }),
  adoptMod: (req: AdoptRequest) => invokeCommand<ModDto>("adopt_mod", { req }),
  /** `withDependencies` applies the dependency closure in the same transaction. */
  setModEnabled: (modId: string, enabled: boolean, withDependencies = false) =>
    invokeCommand<ModDto>(
      "set_mod_enabled",
      withDependencies ? { modId, enabled, withDependencies } : { modId, enabled },
    ),
  previewSetModEnabled: (modId: string, enabled: boolean) =>
    invokeCommand<PlanPreviewDto>("preview_set_mod_enabled", { modId, enabled }),
  addDependency: (modId: string, dependsOnModId: string) =>
    invokeCommand<ModDetailDto>("add_dependency", { modId, dependsOnModId }),
  removeDependency: (modId: string, dependsOnModId: string) =>
    invokeCommand<ModDetailDto>("remove_dependency", { modId, dependsOnModId }),
  /** Stages a ZIP archive (cancellable via `cancelOperation`; emits `import` progress). */
  stageImport: (installationId: string, zipPath: string) =>
    invokeCommand<ImportPreviewDto>("stage_import", { installationId, zipPath }),
  /** `root` is an archive folder, or `null` for the archive root. */
  remapImport: (stagingId: string, root: string | null) =>
    invokeCommand<ImportPreviewDto>("remap_import", { stagingId, root }),
  confirmImport: (req: ConfirmImportRequest) => invokeCommand<ModDto>("confirm_import", { req }),
  discardImport: (stagingId: string) => invokeCommand<void>("discard_import", { stagingId }),
  vaultReport: () => invokeCommand<VaultReportDto>("vault_report"),
  /** Returns the bytes freed. */
  vaultGc: (shas: string[]) => invokeCommand<number>("vault_gc", { shas }),
  previewSetCategory: (installationId: string, category: Category, enabled: boolean) =>
    invokeCommand<PlanPreviewDto>("preview_set_category", { installationId, category, enabled }),
  setCategoryEnabled: (installationId: string, category: Category, enabled: boolean) =>
    invokeCommand<ModDto[]>("set_category_enabled", { installationId, category, enabled }),
  updateMod: (modId: string, name: string, category: Category, notes: string | null) =>
    invokeCommand<ModDto>("update_mod", { modId, name, category, notes }),
  forgetMod: (modId: string) => invokeCommand<void>("forget_mod", { modId }),
  reconcileInstallation: (installationId: string) =>
    invokeCommand<DriftDto[]>("reconcile_installation", { installationId }),
  listProfiles: (installationId: string) =>
    invokeCommand<ProfileDto[]>("list_profiles", { installationId }),
  profileNameSuggestions: () => invokeCommand<string[]>("profile_name_suggestions"),
  createProfileFromCurrent: (installationId: string, name: string) =>
    invokeCommand<ProfileDto>("create_profile_from_current", { installationId, name }),
  getProfile: (profileId: string) => invokeCommand<ProfileDetailDto>("get_profile", { profileId }),
  updateProfileEntries: (profileId: string, entries: ProfileEntryInput[]) =>
    invokeCommand<ProfileDetailDto>("update_profile_entries", { profileId, entries }),
  renameProfile: (profileId: string, name: string) =>
    invokeCommand<ProfileDto>("rename_profile", { profileId, name }),
  deleteProfile: (profileId: string) => invokeCommand<void>("delete_profile", { profileId }),
  /** `profileId` is a profile UUID or `"vanilla"`. */
  previewApplyProfile: (installationId: string, profileId: string) =>
    invokeCommand<PlanPreviewDto>("preview_apply_profile", { installationId, profileId }),
  /** `profileId` is a profile UUID or `"vanilla"`. */
  applyProfile: (installationId: string, profileId: string) =>
    invokeCommand<ModDto[]>("apply_profile", { installationId, profileId }),
  listTransactions: (installationId: string, limit: number, offset: number) =>
    invokeCommand<TransactionPageDto>("list_transactions", { installationId, limit, offset }),
  dashboard: (installationId: string) =>
    invokeCommand<DashboardDto>("dashboard", { installationId }),
  restoreMod: (modId: string) => invokeCommand<ModDto>("restore_mod", { modId }),
  readoptMod: (modId: string) => invokeCommand<ModDto>("readopt_mod", { modId }),
};

/** Id of the built-in profile in which every managed mod is disabled. */
export const VANILLA_PROFILE_ID = "vanilla";

