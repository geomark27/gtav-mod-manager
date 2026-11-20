import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { commands, invokeCommand, VANILLA_PROFILE_ID } from "./ipc";

afterEach(() => clearMocks());

describe("ipc", () => {
  it("returns the command result", async () => {
    mockIPC((cmd) => (cmd === "get_app_status" ? { schemaVersion: 1 } : undefined));
    await expect(commands.getAppStatus()).resolves.toMatchObject({ schemaVersion: 1 });
  });

  it("passes AppError rejections through", async () => {
    const err = { code: "ALREADY_RUNNING", message: "m", hint: "h", details: null };
    mockIPC(() => {
      throw err;
    });
    await expect(invokeCommand("x")).rejects.toEqual(err);
  });

  it("normalizes non-AppError rejections to INTERNAL", async () => {
    mockIPC(() => {
      throw "boom";
    });
    await expect(invokeCommand("x")).rejects.toEqual({
      code: "INTERNAL",
      message: "boom",
      hint: null,
      details: null,
    });
  });

  it("sends camelCase arguments for the mod commands", async () => {
    const calls: Array<[string, unknown]> = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return [];
    });
    await commands.guardStatus("i1");
    await commands.listMods("i1");
    await commands.getMod("m1");
    await commands.previewAdopt("i1", ["a.asi"]);
    await commands.setModEnabled("m1", false);
    await commands.previewSetCategory("i1", "vehicles", true);
    await commands.setCategoryEnabled("i1", "vehicles", false);
    await commands.updateMod("m1", "N", "vehicles", null);
    await commands.forgetMod("m1");
    await commands.reconcileInstallation("i1");
    await commands.adoptMod({
      installationId: "i1",
      relPaths: ["a.asi"],
      name: "N",
      category: "uncategorized",
      notes: null,
      confirmUnknown: false,
    });
    expect(calls).toEqual([
      ["guard_status", { installationId: "i1" }],
      ["list_mods", { installationId: "i1" }],
      ["get_mod", { modId: "m1" }],
      ["preview_adopt", { installationId: "i1", relPaths: ["a.asi"] }],
      ["set_mod_enabled", { modId: "m1", enabled: false }],
      ["preview_set_category", { installationId: "i1", category: "vehicles", enabled: true }],
      ["set_category_enabled", { installationId: "i1", category: "vehicles", enabled: false }],
      ["update_mod", { modId: "m1", name: "N", category: "vehicles", notes: null }],
      ["forget_mod", { modId: "m1" }],
      ["reconcile_installation", { installationId: "i1" }],
      [
        "adopt_mod",
        {
          req: {
            installationId: "i1",
            relPaths: ["a.asi"],
            name: "N",
            category: "uncategorized",
            notes: null,
            confirmUnknown: false,
          },
        },
      ],
    ]);
  });

  it("sends camelCase arguments for the profile, history, dashboard and drift commands", async () => {
    const calls: Array<[string, unknown]> = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return [];
    });
    await commands.listProfiles("i1");
    await commands.profileNameSuggestions();
    await commands.createProfileFromCurrent("i1", "P");
    await commands.getProfile("p1");
    await commands.updateProfileEntries("p1", [{ modId: "m1", desired: "enabled" }]);
    await commands.renameProfile("p1", "Q");
    await commands.deleteProfile("p1");
    await commands.previewApplyProfile("i1", VANILLA_PROFILE_ID);
    await commands.applyProfile("i1", "p1");
    await commands.checkOnlineReadiness("i1");
    await commands.prepareForOnline("i1");
    await commands.vanillaStatus("i1");
    await commands.enterVanilla("i1");
    await commands.restoreFromVanilla("i1");
    await commands.listTransactions("i1", 20, 40);
    await commands.dashboard("i1");
    await commands.restoreMod("m1");
    await commands.readoptMod("m1");
    await commands.detectMods("i1");
    await commands.adoptDetected({ installationId: "i1", items: [], confirmUnknown: true });
    expect(calls).toEqual([
      ["list_profiles", { installationId: "i1" }],
      ["profile_name_suggestions", {}],
      ["create_profile_from_current", { installationId: "i1", name: "P" }],
      ["get_profile", { profileId: "p1" }],
      ["update_profile_entries", { profileId: "p1", entries: [{ modId: "m1", desired: "enabled" }] }],
      ["rename_profile", { profileId: "p1", name: "Q" }],
      ["delete_profile", { profileId: "p1" }],
      ["preview_apply_profile", { installationId: "i1", profileId: "vanilla" }],
      ["apply_profile", { installationId: "i1", profileId: "p1" }],
      ["check_online_readiness", { installationId: "i1" }],
      ["prepare_for_online", { installationId: "i1" }],
      ["vanilla_status", { installationId: "i1" }],
      ["enter_vanilla", { installationId: "i1" }],
      ["restore_from_vanilla", { installationId: "i1" }],
      ["list_transactions", { installationId: "i1", limit: 20, offset: 40 }],
      ["dashboard", { installationId: "i1" }],
      ["restore_mod", { modId: "m1" }],
      ["readopt_mod", { modId: "m1" }],
      ["detect_mods", { installationId: "i1" }],
      ["adopt_detected", { req: { installationId: "i1", items: [], confirmUnknown: true } }],
    ]);
  });

  it("sends camelCase arguments for the import, dependency and vault commands", async () => {
    const calls: Array<[string, unknown]> = [];
    mockIPC((cmd, args) => {
      calls.push([cmd, args]);
      return [];
    });
    await commands.setModEnabled("m1", true, true);
    await commands.previewSetModEnabled("m1", false);
    await commands.addDependency("m1", "m2");
    await commands.removeDependency("m1", "m2");
    await commands.stageImport("i1", "C:/mods/a.zip");
    await commands.remapImport("s1", null);
    await commands.confirmImport({
      stagingId: "s1",
      root: "Mod",
      name: "N",
      category: "uncategorized",
      notes: null,
      enableNow: false,
    });
    await commands.discardImport("s1");
    await commands.vaultReport();
    await commands.vaultGc(["a".repeat(64)]);
    expect(calls).toEqual([
      ["set_mod_enabled", { modId: "m1", enabled: true, withDependencies: true }],
      ["preview_set_mod_enabled", { modId: "m1", enabled: false }],
      ["add_dependency", { modId: "m1", dependsOnModId: "m2" }],
      ["remove_dependency", { modId: "m1", dependsOnModId: "m2" }],
      ["stage_import", { installationId: "i1", zipPath: "C:/mods/a.zip" }],
      ["remap_import", { stagingId: "s1", root: null }],
      [
        "confirm_import",
        {
          req: {
            stagingId: "s1",
            root: "Mod",
            name: "N",
            category: "uncategorized",
            notes: null,
            enableNow: false,
          },
        },
      ],
      ["discard_import", { stagingId: "s1" }],
      ["vault_report", {}],
      ["vault_gc", { shas: ["a".repeat(64)] }],
    ]);
  });
});
