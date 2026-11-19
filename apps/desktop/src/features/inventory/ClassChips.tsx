import type { ClassCountDto } from "../../bindings/ClassCountDto";
import type { Classification } from "../../bindings/Classification";
import { CLASS_INFO, CLASSIFICATIONS } from "./labels";
import styles from "./Inventory.module.css";

export interface ClassChipsProps {
  counts: ClassCountDto[];
  selected: Classification[];
  onToggle: (classification: Classification) => void;
}

/** One toggle chip per classification, with its item count. */
export function ClassChips({ counts, selected, onToggle }: ClassChipsProps) {
  return (
    <div className={styles.chips} role="group" aria-label="Filter by classification">
      {CLASSIFICATIONS.map((c) => {
        const count = counts.find((x) => x.classification === c)?.count ?? 0;
        return (
          <button
            key={c}
            type="button"
            className={styles.chip}
            aria-pressed={selected.includes(c)}
            onClick={() => onToggle(c)}
          >
            {CLASS_INFO[c].label} {count}
          </button>
        );
      })}
    </div>
  );
}
