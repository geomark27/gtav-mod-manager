import type { Category } from "../../bindings/Category";
import { CATEGORIES, CATEGORY_LABEL } from "./labels";
import styles from "./Inventory.module.css";

export interface CategorySelectProps {
  label: string;
  value: Category;
  onChange: (value: Category) => void;
  disabled?: boolean;
}

/** Labeled select over every category. */
export function CategorySelect({ label, value, onChange, disabled }: CategorySelectProps) {
  return (
    <label className={styles.field}>
      <span>{label}</span>
      <select
        className={styles.input}
        value={value}
        disabled={disabled}
        onChange={(e) => {
          const next = CATEGORIES.find((c) => c === e.target.value);
          if (next) onChange(next);
        }}
      >
        {CATEGORIES.map((c) => (
          <option key={c} value={c}>
            {CATEGORY_LABEL[c]}
          </option>
        ))}
      </select>
    </label>
  );
}
