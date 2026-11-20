import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppError } from "../../bindings/AppError";
import type { DriftDto } from "../../bindings/DriftDto";
import type { GuardStatusDto } from "../../bindings/GuardStatusDto";
import type { ModDetailDto } from "../../bindings/ModDetailDto";
import type { ModDto } from "../../bindings/ModDto";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import type { VanillaStatusDto } from "../../bindings/VanillaStatusDto";

/** Vanilla status of an installation without managed mods (the panel renders nothing). */
export const NO_VANILLA: VanillaStatusDto = {
  active: false,
  restorePoint: null,
  modsTotal: 0,
  modsInGame: 0,
  unmanagedFiles: null,
};

/** Builds a mod for tests. */
export function makeMod(over: Partial<ModDto> = {}): ModDto {
  return {
    id: "mod-1",
    name: "Alpha",
    category: "scripts_gameplay",
    state: "disabled",
    notes: null,
    entryCount: 2,
    totalSize: 2048,
    conflictsWith: [],
    dependsOnCount: 0,
    source: { kind: "manual", detail: null, sha256: null, version: null },
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...over,
  };
}

export const OK_GUARD: GuardStatusDto = {
  gameRunning: [],
  writable: true,
  recoveryRequired: false,
  installationMissing: false,
};

export const APP_ERROR: AppError = {
  code: "DRIFT_DETECTED",
  message: "Files changed outside the app.",
  hint: "Check again first.",
  details: null,
};

export interface ModsMock {
  mods: ModDto[];
  guard: GuardStatusDto;
  drift: DriftDto[];
  preview: PlanPreviewDto;
  detail: (id: string) => ModDetailDto;
  /** Overrides per command; may be async or throw an AppError. */
  handlers: Record<string, (args: Record<string, unknown>) => unknown>;
  calls: Array<{ cmd: string; args: Record<string, unknown> }>;
}

/** Installs a stateful IPC mock for the Mods page; returns it so tests can change it. */
export function mockModsIpc(initial: Partial<ModsMock> = {}): ModsMock {
  const mock: ModsMock = {
    mods: [],
    guard: OK_GUARD,
    drift: [],
    preview: { modsAffected: [], actions: [], bytesToCopy: 0 },
    detail: (id) => {
      const mod = mock.mods.find((m) => m.id === id) ?? makeMod({ id });
      return {
        ...mod,
        dependsOn: [],
        requiredBy: [],
        entries: [
          { relPath: "scripts/a.asi", size: 1024, sha256: "abcdef0123456789", fileVersion: null },
          { relPath: "scripts/a.ini", size: 1024, sha256: "0123456789abcdef", fileVersion: null },
        ],
      };
    },
    handlers: {},
    calls: [],
    ...initial,
  };
  mockIPC((cmd, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    mock.calls.push({ cmd, args });
    const override = mock.handlers[cmd];
    if (override) return override(args);
    switch (cmd) {
      case "guard_status":
        return mock.guard;
      case "reconcile_installation":
        return mock.drift;
      case "list_mods":
        return mock.mods;
      case "get_mod":
        return mock.detail(String(args.modId));
      case "preview_set_mod_enabled": {
        const mod = mock.mods.find((m) => m.id === String(args.modId));
        return {
          modsAffected: mod
            ? [{ modId: mod.id, name: mod.name, from: mod.state, to: args.enabled ? "enabled" : "disabled" }]
            : [],
          actions: [],
          bytesToCopy: 0,
        } satisfies PlanPreviewDto;
      }
      case "preview_set_category":
        return mock.preview;
      case "vanilla_status":
        return NO_VANILLA;
      default:
        return undefined;
    }
  });
  return mock;
}
