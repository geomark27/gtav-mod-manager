import type { GuardStatusDto } from "../../bindings/GuardStatusDto";
import { guardNotice } from "./blocking";

const OK: GuardStatusDto = {
  gameRunning: [],
  writable: true,
  recoveryRequired: false,
  installationMissing: false,
};

describe("guardNotice", () => {
  it("is null when writes are allowed", () => {
    expect(guardNotice(OK)).toBeNull();
  });

  it("names the running processes", () => {
    expect(guardNotice({ ...OK, gameRunning: ["GTA5.exe", "Steam.exe"] })).toEqual({
      severity: "warning",
      text: "The game or a launcher is running (GTA5.exe, Steam.exe). Close it to change mods.",
    });
  });

  it("reports a missing folder, recovery and a read-only folder", () => {
    expect(guardNotice({ ...OK, installationMissing: true })?.text).toMatch(/not found/);
    const recovery = guardNotice({ ...OK, recoveryRequired: true });
    expect(recovery?.severity).toBe("danger");
    expect(guardNotice({ ...OK, writable: false })?.text).toMatch(/not writable/);
  });

  it("gives the running game priority", () => {
    const n = guardNotice({ ...OK, gameRunning: ["GTA5.exe"], recoveryRequired: true });
    expect(n?.severity).toBe("warning");
  });
});
