import type { AppError } from "../../bindings/AppError";
import type { Category } from "../../bindings/Category";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { formatBytes } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import { CATEGORY_LABEL } from "../inventory/labels";
import styles from "./Mods.module.css";
import { usePreviewCategory } from "./useMods";

export interface GroupSwitchDialogProps {
  installationId: string;
  category: Category;
  /** The state the category would be switched to. */
  enable: boolean;
  open: boolean;
  applying: boolean;
  error: AppError | null;
  onApply: () => void;
  onCancel: () => void;
}

/** Confirmation with the dry run of a category switch (D-08). */
export function GroupSwitchDialog({
  installationId,
  category,
  enable,
  open,
  applying,
  error,
  onApply,
  onCancel,
}: GroupSwitchDialogProps) {
  const preview = usePreviewCategory(installationId, category, enable, open);
  const data = preview.data;
  const nothing = data !== undefined && data.modsAffected.length === 0;
  return (
    <Modal
      open={open}
      onOpenChange={(next) => {
        if (!next) onCancel();
      }}
      dismissible={!applying}
      title={`${enable ? "Enable" : "Disable"} ${CATEGORY_LABEL[category]}`}
      description="Review what will change in the game folder before applying."
    >
      {preview.isPending ? <p className={styles.muted}>Calculating changes…</p> : null}
      {preview.error ? <ErrorMessage error={preview.error} /> : null}
      {data ? (
        <div className={styles.stack}>
          {nothing ? (
            <p>Nothing to change: every mod in this category is already {enable ? "enabled" : "disabled"}.</p>
          ) : (
            <>
              <p>
                {data.modsAffected.length} {data.modsAffected.length === 1 ? "mod" : "mods"} affected
                · {data.actions.length} file {data.actions.length === 1 ? "action" : "actions"} ·{" "}
                {formatBytes(data.bytesToCopy)} to copy
              </p>
              <ul className={styles.plainList}>
                {data.modsAffected.map((m) => (
                  <li key={m.modId}>{m.name}</li>
                ))}
              </ul>
            </>
          )}
        </div>
      ) : null}
      {error ? <ErrorMessage error={error} /> : null}
      <div className={styles.actions}>
        <Button onClick={onCancel} disabled={applying}>
          Cancel
        </Button>
        <Button
          variant="primary"
          loading={applying}
          disabled={!data || nothing}
          onClick={onApply}
        >
          Apply
        </Button>
      </div>
    </Modal>
  );
}
