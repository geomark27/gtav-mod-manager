import { sourceLabel, stateProblem, switchState } from "./modLabels";

describe("switchState", () => {
  it("derives from the server state", () => {
    expect(switchState("enabled", false, null)).toBe("on");
    expect(switchState("disabled", false, null)).toBe("off");
    expect(switchState("drifted", false, null)).toBe("error");
    expect(switchState("error", false, null)).toBe("error");
  });

  it("pending wins over blocked, blocked over state", () => {
    expect(switchState("enabled", true, "x")).toBe("pending");
    expect(switchState("enabled", false, "x")).toBe("blocked");
  });
});

describe("stateProblem", () => {
  it("explains drifted and error only", () => {
    expect(stateProblem("drifted")).toBe("Files changed outside the app");
    expect(stateProblem("error")).not.toBeNull();
    expect(stateProblem("enabled")).toBeNull();
    expect(stateProblem("disabled")).toBeNull();
  });
});

describe("sourceLabel", () => {
  const at = "2026-11-19T14:03:00Z";
  const base = { detail: null, sha256: null, version: null };
  it("describes every kind of source", () => {
    expect(sourceLabel({ ...base, kind: "manual" }, at, "UTC")).toBe(
      "Adopted by hand on Nov 19, 2026, 2:03 PM",
    );
    expect(
      sourceLabel({ ...base, kind: "zip_import", detail: "Rampage.zip" }, at, "UTC"),
    ).toBe("Imported from Rampage.zip on Nov 19, 2026, 2:03 PM");
    expect(sourceLabel({ ...base, kind: "zip_import" }, at, "UTC")).toMatch(
      /^Imported from a ZIP archive on /,
    );
    expect(
      sourceLabel({ ...base, kind: "recognized", detail: "rampage-trainer", version: "4.2" }, at, "UTC"),
    ).toMatch(/^Recognized as rampage-trainer 4.2 on /);
    expect(sourceLabel({ ...base, kind: "grouped" }, at, "UTC")).toMatch(/^Grouped from files/);
  });
});
