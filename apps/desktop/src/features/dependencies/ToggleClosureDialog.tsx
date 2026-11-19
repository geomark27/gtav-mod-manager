import type { AppError } from "../../bindings/AppError";
import type { PlanPreviewDto } from "../../bindings/PlanPreviewDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "../mods/Mods.module.css";

export interface ToggleClosureDialogProps {
  modName: string;
  /** The state the mod would be switched to. */
  enable: boolean;
  preview: PlanPreviewDto;
  applying: boolean;
  error: AppError | null;
  onApply: () => void;
  onCancel: () => void;
}

/** Confirmation shown when switching a mod also changes other mods it depends on / that depend on it. */
export function ToggleClosureDialog({
  modName,
  enable,
  preview,
  applying,
  error,
  onApply,
  onCancel,
}: ToggleClosureDialogProps) {
  const others = preview.modsAffected.filter((m) => m.name !== modName);
  const verb = enable ? "enable" : "disable";
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onCancel();
      }}
      dismissible={!applying}
      title={`${enable ? "Enable" : "Disable"} ${modName}`}
      description="This change also affects other mods because of their requirements."
    >
      <div className={styles.stack}>
        <p>Also {verb}:</p>
        <ul className={styles.plainList}>
          {others.map((m) => (
            <li key={m.modId}>{m.name}</li>
          ))}
        </ul>
        {error ? <ErrorMessage error={error} /> : null}
        <div className={styles.actions}>
          <Button onClick={onCancel} disabled={applying}>
            Cancel
          </Button>
          <Button variant="primary" loading={applying} onClick={onApply}>
            Apply to all
          </Button>
        </div>
      </div>
    </Modal>
  );
}
