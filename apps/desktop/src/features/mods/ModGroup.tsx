import { useState } from "react";
import type { Category } from "../../bindings/Category";
import type { ModDto } from "../../bindings/ModDto";
import { ModSwitch } from "../../components/ModSwitch";
import { CATEGORY_LABEL } from "../inventory/labels";
import { GroupSwitchDialog } from "./GroupSwitchDialog";
import { ModRow } from "./ModRow";
import styles from "./Mods.module.css";
import { useSetCategoryEnabled } from "./useMods";

export interface ModGroupProps {
  installationId: string;
  category: Category;
  /** Every mod of the category (the switch acts on all of them). */
  all: ModDto[];
  /** The rows to show after filtering. */
  visible: ModDto[];
  blockedReason: string | null;
  checking: boolean;
  onOpen: (mod: ModDto) => void;
  onCheckAgain: () => void;
}

/** A category: header with the group switch and its mod rows. */
export function ModGroup({
  installationId,
  category,
  all,
  visible,
  blockedReason,
  checking,
  onOpen,
  onCheckAgain,
}: ModGroupProps) {
  const [target, setTarget] = useState<boolean | null>(null);
  const apply = useSetCategoryEnabled(installationId);
  const enabledCount = all.filter((m) => m.state === "enabled").length;
  const allEnabled = enabledCount === all.length;
  const problem = all.some((m) => m.state === "drifted" || m.state === "error");
  const label = CATEGORY_LABEL[category];

  const state = apply.isPending
    ? "pending"
    : blockedReason !== null
      ? "blocked"
      : problem
        ? "error"
        : allEnabled
          ? "on"
          : "off";

  return (
    <section className={styles.group} aria-label={label}>
      <header className={styles.groupHeader}>
        <h2 className={styles.groupName}>{label}</h2>
        <span className={styles.muted}>
          {enabledCount}/{all.length} enabled
        </span>
        <span className={styles.switchSlot}>
          <ModSwitch
            label={`${label} (all)`}
            state={state}
            blockedReason={blockedReason ?? undefined}
            onRequestChange={(next) => {
              apply.reset();
              setTarget(next);
            }}
          />
        </span>
      </header>
      <ul className={styles.list}>
        {visible.map((mod) => (
          <ModRow
            key={mod.id}
            installationId={installationId}
            mod={mod}
            blockedReason={blockedReason}
            checking={checking}
            onOpen={onOpen}
            onCheckAgain={onCheckAgain}
          />
        ))}
      </ul>
      {target !== null ? (
        <GroupSwitchDialog
          installationId={installationId}
          category={category}
          enable={target}
          open
          applying={apply.isPending}
          error={apply.error}
          onApply={() =>
            apply.mutate({ category, enabled: target }, { onSuccess: () => setTarget(null) })
          }
          onCancel={() => setTarget(null)}
        />
      ) : null}
    </section>
  );
}
