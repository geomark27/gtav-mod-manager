import { mockIPC } from "@tauri-apps/api/mocks";
import type { GuardStatusDto } from "../../bindings/GuardStatusDto";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import type { ProfileDetailDto } from "../../bindings/ProfileDetailDto";
import type { ProfileDto } from "../../bindings/ProfileDto";
import type { ModDto } from "../../bindings/ModDto";
import { OK_GUARD, makeMod } from "../mods/testing";

/** Builds a profile for tests. */
export function makeProfile(over: Partial<ProfileDto> = {}): ProfileDto {
  return {
    id: "p1",
    name: "Story",
    entryCount: 2,
    builtIn: false,
    kind: "user",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...over,
  };
}

export const VANILLA = makeProfile({ id: "vanilla", name: "Vanilla", builtIn: true, entryCount: 3 });

export interface ProfilesMock {
  profiles: ProfileDto[];
  suggestions: string[];
  guard: GuardStatusDto;
  preview: PlanPreviewDto;
  detail: ProfileDetailDto;
  mods: ModDto[];
  handlers: Record<string, (args: Record<string, unknown>) => unknown>;
  calls: Array<{ cmd: string; args: Record<string, unknown> }>;
}

/** Installs an IPC mock for the Profiles page. */
export function mockProfilesIpc(initial: Partial<ProfilesMock> = {}): ProfilesMock {
  const mock: ProfilesMock = {
    profiles: [VANILLA, makeProfile()],
    suggestions: ["Story", "Clean"],
    guard: OK_GUARD,
    preview: {
      modsAffected: [{ modId: "m1", name: "Alpha", from: "disabled", to: "enabled" }],
      actions: [],
      bytesToCopy: 2048,
    },
    detail: {
      ...makeProfile(),
      entries: [
        { modId: "m1", modName: "Alpha", desired: "enabled" },
        { modId: "m2", modName: "Beta", desired: "disabled" },
      ],
    },
    mods: [
      makeMod({ id: "m1", name: "Alpha" }),
      makeMod({ id: "m2", name: "Beta" }),
      makeMod({ id: "m3", name: "Gamma" }),
    ],
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
      case "list_profiles":
        return mock.profiles;
      case "profile_name_suggestions":
        return mock.suggestions;
      case "preview_apply_profile":
        return mock.preview;
      case "list_mods":
        return mock.mods;
      case "get_profile":
        return mock.detail;
      default:
        return undefined;
    }
  });
  return mock;
}
