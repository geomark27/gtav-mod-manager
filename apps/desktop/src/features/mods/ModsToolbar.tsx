import type { ReactNode } from "react";
import type { Category } from "../../bindings/Category";
import type { ModDto } from "../../bindings/ModDto";
import { CATEGORIES, CATEGORY_LABEL } from "../inventory/labels";
import styles from "./Mods.module.css";

export type StateFilter = "all" | "enabled" | "disabled" | "attention";
export type CategoryFilter = Category | "all";

export interface ModFilters {
  search: string;
  state: StateFilter;
  category: CategoryFilter;
}

export const NO_FILTERS: ModFilters = { search: "", state: "all", category: "all" };

const STATE_OPTIONS: ReadonlyArray<{ value: StateFilter; label: string }> = [
  { value: "all", label: "All" },
  { value: "enabled", label: "Enabled" },
  { value: "disabled", label: "Disabled" },
  { value: "attention", label: "Needs attention" },
];

/** Whether a mod passes the filters. */
export function matchesFilters(mod: ModDto, f: ModFilters): boolean {
  const q = f.search.trim().toLowerCase();
  if (q !== "" && !mod.name.toLowerCase().includes(q)) return false;
  if (f.category !== "all" && mod.category !== f.category) return false;
  switch (f.state) {
    case "all":
      return true;
    case "enabled":
      return mod.state === "enabled";
    case "disabled":
      return mod.state === "disabled";
    case "attention":
      return mod.state === "drifted" || mod.state === "error";
  }
}

export interface ModsToolbarProps {
  filters: ModFilters;
  onChange: (filters: ModFilters) => void;
  /** Extra controls shown at the end of the toolbar (for example Import ZIP). */
  actions?: ReactNode;
}

/** Search and filters of the Mods page. */
export function ModsToolbar({ filters, onChange, actions }: ModsToolbarProps) {
  return (
    <div className={styles.toolbar}>
      <input
        type="search"
        className={styles.input}
        aria-label="Search mods"
        placeholder="Search by name"
        value={filters.search}
        onChange={(e) => onChange({ ...filters, search: e.target.value })}
      />
      <label className={styles.inline}>
        <span>State</span>
        <select
          className={styles.input}
          value={filters.state}
          onChange={(e) => {
            const next = STATE_OPTIONS.find((o) => o.value === e.target.value);
            if (next) onChange({ ...filters, state: next.value });
          }}
        >
          {STATE_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.inline}>
        <span>Category</span>
        <select
          className={styles.input}
          value={filters.category}
          onChange={(e) => {
            const next = CATEGORIES.find((c) => c === e.target.value);
            onChange({ ...filters, category: next ?? "all" });
          }}
        >
          <option value="all">All categories</option>
          {CATEGORIES.map((c) => (
            <option key={c} value={c}>
              {CATEGORY_LABEL[c]}
            </option>
          ))}
        </select>
      </label>
      {actions}
    </div>
  );
}
