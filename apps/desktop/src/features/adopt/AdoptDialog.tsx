import { useState } from "react";
import type { AdoptPreviewDto } from "../../bindings/AdoptPreviewDto";
import type { AppError } from "../../bindings/AppError";
import type { Category } from "../../bindings/Category";
import type { ModDto } from "../../bindings/ModDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { formatBytes } from "../../lib/format";
import { useProgress } from "../../lib/progress";
import { ErrorMessage } from "../installations/ErrorMessage";
import { CategorySelect } from "../inventory/CategorySelect";
import inventoryStyles from "../inventory/Inventory.module.css";
import styles from "./Adopt.module.css";
import { useAdoptMod, useAdoptPreview } from "./useAdopt";

export interface AdoptDialogProps {
  installationId: string;
  relPaths: string[];
  onClose: () => void;
  onAdopted: (mod: ModDto) => void;
}

/** Adopts selected candidate/unknown files as one mod. */
export function AdoptDialog({ installationId, relPaths, onClose, onAdopted }: AdoptDialogProps) {
  const preview = useAdoptPreview(installationId, relPaths);
  const adopt = useAdoptMod(installationId);
  const running = adopt.isPending;
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      dismissible={!running}
      title="Adopt as mod"
      description="The selected files become one managed mod that you can switch on and off."
    >
      {preview.isPending ? <p className={styles.muted}>Checking the selection…</p> : null}
      {preview.error ? (
        <>
          <ErrorMessage error={preview.error} />
          <div className={styles.actions}>
            <Button onClick={onClose}>Close</Button>
          </div>
        </>
      ) : null}
      {preview.data ? (
        <AdoptForm
          preview={preview.data}
          running={running}
          error={adopt.error}
          onCancel={onClose}
          onSubmit={(v) =>
            adopt.mutate(
              {
                installationId,
                relPaths: preview.data.files.map((f) => f.relPath),
                name: v.name,
                category: v.category,
                notes: v.notes,
                confirmUnknown: v.confirmUnknown,
              },
              { onSuccess: onAdopted },
            )
          }
        />
      ) : null}
    </Modal>
  );
}

interface FormValues {
  name: string;
  category: Category;
  notes: string | null;
  confirmUnknown: boolean;
}

interface FormProps {
  preview: AdoptPreviewDto;
  running: boolean;
  error: AppError | null;
  onCancel: () => void;
  onSubmit: (values: FormValues) => void;
}

function AdoptForm({ preview, running, error, onCancel, onSubmit }: FormProps) {
  // `null` = untouched: the suggestion from the preview is used.
  const [name, setName] = useState<string | null>(null);
  const [category, setCategory] = useState<Category | null>(null);
  const [notes, setNotes] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const progress = useProgress("adopt");

  const nameValue = name ?? preview.suggestedName;
  const categoryValue = category ?? preview.suggestedCategory;
  const valid =
    preview.files.length > 0 &&
    nameValue.trim() !== "" &&
    (!preview.includesUnknown || confirmed);

  return (
    <form
      className={styles.stack}
      onSubmit={(e) => {
        e.preventDefault();
        if (!valid || running) return;
        onSubmit({
          name: nameValue.trim(),
          category: categoryValue,
          notes: notes.trim() === "" ? null : notes,
          confirmUnknown: preview.includesUnknown && confirmed,
        });
      }}
    >
      <p>
        {preview.files.length} {preview.files.length === 1 ? "file" : "files"} ·{" "}
        {formatBytes(preview.totalSize)}
      </p>
      {preview.files.length > 0 ? (
        <ul className={styles.fileList} aria-label="Files to adopt">
          {preview.files.map((f) => (
            <li key={f.relPath}>
              <span className={styles.path}>{f.relPath}</span>{" "}
              <span className={styles.muted}>{formatBytes(f.size)}</span>
            </li>
          ))}
        </ul>
      ) : (
        <p>None of the selected files can be adopted.</p>
      )}
      {preview.blocked.length > 0 ? (
        <div className={styles.stack}>
          <p>Blocked files</p>
          <ul className={styles.fileList} aria-label="Blocked files">
            {preview.blocked.map((b) => (
              <li key={b.relPath}>
                <span className={styles.path}>{b.relPath}</span>{" "}
                <span className={styles.muted}>{b.reason}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      <label className={inventoryStyles.field}>
        <span>Name</span>
        <input
          className={inventoryStyles.input}
          value={nameValue}
          disabled={running}
          onChange={(e) => setName(e.target.value)}
        />
      </label>
      <CategorySelect
        label="Category"
        value={categoryValue}
        disabled={running}
        onChange={setCategory}
      />
      <label className={inventoryStyles.field}>
        <span>Notes</span>
        <textarea
          className={inventoryStyles.input}
          rows={3}
          value={notes}
          disabled={running}
          onChange={(e) => setNotes(e.target.value)}
        />
      </label>

      {preview.includesUnknown ? (
        <label className={styles.confirm}>
          <input
            type="checkbox"
            checked={confirmed}
            disabled={running}
            onChange={(e) => setConfirmed(e.target.checked)}
          />
          <span>I confirm these unknown files are not part of the game.</span>
        </label>
      ) : null}

      {running ? (
        <div className={inventoryStyles.stack} role="status" aria-label="Adopt progress">
          <div className={inventoryStyles.bar} role="progressbar" aria-label="Adopting" />
          <span>
            {progress && !progress.done
              ? `${progress.filesSeen} files processed · ${formatBytes(progress.bytesHashed)}`
              : "Adopting…"}
          </span>
        </div>
      ) : null}
      {error ? <ErrorMessage error={error} /> : null}

      <div className={styles.actions}>
        <Button onClick={onCancel} disabled={running}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" loading={running} disabled={!valid}>
          Adopt
        </Button>
      </div>
    </form>
  );
}
