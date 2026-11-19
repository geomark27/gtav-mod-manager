import { useState } from "react";
import type { ModDetailDto } from "../../bindings/ModDetailDto";
import { Button } from "../../components/Button";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "../mods/Mods.module.css";
import { useMods } from "../mods/useMods";
import { useAddDependency, useRemoveDependency } from "./useDependencies";

export interface DependenciesSectionProps {
  installationId: string;
  detail: ModDetailDto;
}

/** "Requires" (editable) and "Required by" (read-only) lists of one mod. */
export function DependenciesSection({ installationId, detail }: DependenciesSectionProps) {
  const mods = useMods(installationId, true);
  const add = useAddDependency(installationId, detail.id);
  const remove = useRemoveDependency(installationId, detail.id);
  const [choice, setChoice] = useState("");

  const taken = new Set([
    detail.id,
    ...detail.dependsOn.map((r) => r.modId),
    ...detail.requiredBy.map((r) => r.modId),
  ]);
  const options = (mods.data ?? []).filter((m) => !taken.has(m.id));
  const error = add.error ?? remove.error;

  return (
    <div className={styles.stack}>
      <h3 className={styles.heading}>Requires</h3>
      {detail.dependsOn.length === 0 ? (
        <p className={styles.muted}>This mod does not require other mods.</p>
      ) : (
        <ul className={styles.depList}>
          {detail.dependsOn.map((r) => (
            <li key={r.modId} className={styles.depItem}>
              <span>{r.name}</span>
              <Button
                loading={remove.isPending && remove.variables === r.modId}
                disabled={add.isPending || remove.isPending}
                aria-label={`Remove requirement ${r.name}`}
                onClick={() => remove.mutate(r.modId)}
              >
                Remove
              </Button>
            </li>
          ))}
        </ul>
      )}
      <div className={styles.inlineForm}>
        <label className={styles.field}>
          <span>Add requirement</span>
          <select
            className={styles.input}
            value={choice}
            onChange={(e) => setChoice(e.target.value)}
          >
            <option value="">Choose a mod…</option>
            {options.map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
              </option>
            ))}
          </select>
        </label>
        <Button
          loading={add.isPending}
          disabled={choice === "" || remove.isPending}
          onClick={() => add.mutate(choice, { onSuccess: () => setChoice("") })}
        >
          Add
        </Button>
      </div>
      {error ? <ErrorMessage error={error} /> : null}
      <h3 className={styles.heading}>Required by</h3>
      {detail.requiredBy.length === 0 ? (
        <p className={styles.muted}>No other mod requires this one.</p>
      ) : (
        <ul className={styles.plainList}>
          {detail.requiredBy.map((r) => (
            <li key={r.modId}>{r.name}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
