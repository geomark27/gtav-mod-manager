import type { Evidence } from "../../bindings/Evidence";
import { CLASS_INFO, CLASSIFICATIONS, evidenceText } from "./labels";

describe("evidenceText", () => {
  it.each<[Evidence, string]>([
    [{ kind: "catalog_original" }, "In game catalog"],
    [{ kind: "baseline_match" }, "Matches baseline"],
    [{ kind: "baseline_mismatch" }, "Differs from baseline"],
    [{ kind: "signed_by", publisher: "Rockstar Games, Inc." }, "Signed by Rockstar Games, Inc."],
    [{ kind: "unsigned" }, "Not signed"],
    [{ kind: "mod_pattern", pattern: "ASI plugin" }, "Matches: ASI plugin"],
    [{ kind: "reparse_point" }, "Symbolic link / junction"],
    [{ kind: "hard_linked" }, "Hard-linked"],
    [{ kind: "managed_by", mod_id: "m" }, "Managed by a mod"],
  ])("describes %j", (evidence, text) => {
    expect(evidenceText(evidence)).toBe(text);
  });
});

describe("classification labels", () => {
  it("labels every classification", () => {
    expect(CLASSIFICATIONS.map((c) => CLASS_INFO[c].label)).toEqual([
      "Original",
      "Modified original",
      "Candidate",
      "Unknown",
      "Managed",
      "Drifted",
      "Engine file",
    ]);
  });
});
