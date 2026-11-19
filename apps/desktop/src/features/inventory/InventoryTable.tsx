import * as Checkbox from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import type { InventoryItemDto } from "../../bindings/InventoryItemDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { formatBytes, formatDateTime } from "../../lib/format";
import { CATEGORY_LABEL, CLASS_INFO, evidenceText } from "./labels";
import styles from "./Inventory.module.css";

export interface InventoryTableProps {
  items: InventoryItemDto[];
  total: number;
  offset: number;
  pageSize: number;
  /** Text shown when there are no rows. */
  emptyText: string;
  onOffsetChange: (offset: number) => void;
  /** Row selection; only `candidate` and `unknown` rows can be selected. */
  selection?: {
    selected: ReadonlySet<string>;
    onToggle: (relPath: string) => void;
  };
}

/** Whether a classification can be adopted as a mod. */
export function isSelectable(c: InventoryItemDto["classification"]): boolean {
  return c === "candidate" || c === "unknown";
}

/** Classified files with their evidence, and pagination. */
export function InventoryTable({
  items,
  total,
  offset,
  pageSize,
  emptyText,
  onOffsetChange,
  selection,
}: InventoryTableProps) {
  if (items.length === 0) return <p className={styles.muted}>{emptyText}</p>;
  const first = offset + 1;
  const last = offset + items.length;
  return (
    <div className={styles.stack}>
      <div className={styles.tableWrap}>
        <table className={styles.table}>
          <thead>
            <tr>
              {selection ? <th aria-label="Selection" /> : null}
              <th>Path</th>
              <th>Class</th>
              <th>Suggested category</th>
              <th>Size</th>
              <th>Evidence</th>
              <th>First seen</th>
            </tr>
          </thead>
          <tbody>
            {items.map((item) => (
              <tr key={item.relPath}>
                {selection ? (
                  <td>
                    {isSelectable(item.classification) ? (
                      <Checkbox.Root
                        className={styles.selectBox}
                        aria-label={`Select ${item.relPath}`}
                        checked={selection.selected.has(item.relPath)}
                        onCheckedChange={() => selection.onToggle(item.relPath)}
                      >
                        <Checkbox.Indicator>
                          <Check size={12} aria-hidden="true" />
                        </Checkbox.Indicator>
                      </Checkbox.Root>
                    ) : null}
                  </td>
                ) : null}
                <td className={styles.path}>{item.relPath}</td>
                <td>
                  <Badge tone={CLASS_INFO[item.classification].tone}>
                    {CLASS_INFO[item.classification].label}
                  </Badge>
                </td>
                <td>
                  {item.suggestedCategory ? CATEGORY_LABEL[item.suggestedCategory] : "—"}
                </td>
                <td className={styles.numeric}>{formatBytes(item.size)}</td>
                <td>
                  {item.evidence.length === 0
                    ? "—"
                    : item.evidence.map((e) => evidenceText(e)).join(" · ")}
                </td>
                <td className={styles.numeric}>{formatDateTime(item.firstSeenAt)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className={styles.pager}>
        <Button
          onClick={() => onOffsetChange(Math.max(0, offset - pageSize))}
          disabled={offset === 0}
        >
          Previous
        </Button>
        <span className={styles.muted}>
          Showing {first}–{last} of {total}
        </span>
        <Button onClick={() => onOffsetChange(offset + pageSize)} disabled={last >= total}>
          Next
        </Button>
      </div>
    </div>
  );
}
