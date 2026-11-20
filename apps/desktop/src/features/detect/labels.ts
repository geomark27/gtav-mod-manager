import type { BadgeTone } from "../../components/Badge";
import type { DetectionConfidence } from "../../bindings/DetectionConfidence";
import type { RequirementDto } from "../../bindings/RequirementDto";

export const CONFIDENCE_INFO: Record<
  DetectionConfidence,
  { label: string; tone: BadgeTone; hint: string }
> = {
  high: {
    label: "Known mod",
    tone: "success",
    hint: "Its file names match a known mod.",
  },
  medium: {
    label: "Likely match",
    tone: "info",
    hint: "Its files match a known mod's naming pattern. Check the list before adopting.",
  },
  low: {
    label: "Grouped",
    tone: "warning",
    hint: "Related files grouped by folder or name. Check the list and the name before adopting.",
  },
};

/** One requirement in words, with the tone of its badge. */
export function requirementInfo(r: RequirementDto): { text: string; tone: BadgeTone } {
  switch (r.status) {
    case "managed":
      return { text: `${r.name}: managed as “${r.modName ?? r.name}”`, tone: "success" };
    case "detected":
      return { text: `${r.name}: detected here`, tone: "info" };
    case "missing":
      return { text: `${r.name}: not found`, tone: "danger" };
  }
}
