import type { BadgeTone } from "../../components/Badge";
import type { Category } from "../../bindings/Category";
import type { Classification } from "../../bindings/Classification";
import type { Evidence } from "../../bindings/Evidence";

/** Classifications in display order. */
export const CLASSIFICATIONS: readonly Classification[] = [
  "original",
  "original_modified",
  "candidate",
  "unknown",
  "managed",
  "drifted",
  "engine_artifact",
];

export const CLASS_INFO: Record<Classification, { label: string; tone: BadgeTone }> = {
  original: { label: "Original", tone: "success" },
  original_modified: { label: "Modified original", tone: "warning" },
  candidate: { label: "Candidate", tone: "info" },
  unknown: { label: "Unknown", tone: "warning" },
  managed: { label: "Managed", tone: "success" },
  drifted: { label: "Drifted", tone: "danger" },
  engine_artifact: { label: "Engine file", tone: "neutral" },
};

export const CATEGORY_LABEL: Record<Category, string> = {
  graphics_textures: "Graphics & Textures",
  vehicles: "Vehicles",
  scripts_gameplay: "Scripts & Gameplay",
  maps_buildings: "Maps & Buildings",
  characters_clothing: "Characters & Clothing",
  sounds_music: "Sounds & Music",
  uncategorized: "Uncategorized",
};

/** Short readable text for one piece of evidence. */
export function evidenceText(e: Evidence): string {
  switch (e.kind) {
    case "catalog_original":
      return "In game catalog";
    case "baseline_match":
      return "Matches baseline";
    case "baseline_mismatch":
      return "Differs from baseline";
    case "signed_by":
      return `Signed by ${e.publisher}`;
    case "unsigned":
      return "Not signed";
    case "mod_pattern":
      return `Matches: ${e.pattern}`;
    case "reparse_point":
      return "Symbolic link / junction";
    case "hard_linked":
      return "Hard-linked";
    case "managed_by":
      return "Managed by a mod";
  }
}

/** Categories in display order. */
export const CATEGORIES = Object.keys(CATEGORY_LABEL) as Category[];
