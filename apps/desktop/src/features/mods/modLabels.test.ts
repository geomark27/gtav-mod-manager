import { stateProblem, switchState } from "./modLabels";

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
