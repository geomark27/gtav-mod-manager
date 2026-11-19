import { useState } from "react";
import type { Category } from "../../bindings/Category";
import type { ImportMappedDto } from "../../bindings/ImportMappedDto";
import type { ImportPreviewDto } from "../../bindings/ImportPreviewDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { formatBytes } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import { CategorySelect } from "../inventory/CategorySelect";
import styles from "../mods/Mods.module.css";
import { archiveFolders } from "./archiveFolders";
import { useConfirmImport, useDiscardImport, useRemapImport } from "./useImport";

/** Hint the backend attaches when the mod was imported but enabling it failed. */
const IMPORTED_NOT_ENABLED = "The mod was imported but not enabled.";

export interface ImportDialogProps {
  installationId: string;
  initial: ImportPreviewDto;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
  onClose: () => void;
}

function TargetBadge({ target }: { target: ImportMappedDto }) {
  if (target.protected) return <Badge tone="danger">Game file — blocked</Badge>;
  if (target.conflict) return <Badge tone="warning">Conflict</Badge>;
  return <Badge tone="success">New</Badge>;
}

/** Review of a staged archive: install root, mapped targets and the final import. */
export function ImportDialog({ installationId, initial, blockedReason, onClose }: ImportDialogProps) {
  const [preview, setPreview] = useState(initial);
  const [name, setName] = useState(initial.suggestedName);
  const [category, setCategory] = useState<Category>(initial.suggestedCategory);
  const [notes, setNotes] = useState("");
  const [enableChoice, setEnableChoice] = useState<boolean | null>(null);
  const remap = useRemapImport(initial.stagingId);
  const confirm = useConfirmImport(installationId);
  const discard = useDiscardImport();

  const anyConflict = preview.mapped.some((m) => m.conflict);
  const anyProtected = preview.mapped.some((m) => m.protected);
  const nothingMapped = preview.mapped.length === 0;
  const enableNow = enableChoice ?? !anyConflict;
  const trimmed = name.trim();
  const busy = remap.isPending || confirm.isPending || discard.isPending;

  const folders = archiveFolders(preview.files.map((f) => f.archivePath));
  if (preview.root !== null && !folders.includes(preview.root)) folders.push(preview.root);

  // After "imported but not enabled" the staging is gone: only closing makes sense.
  const importedOnly = confirm.error?.hint === IMPORTED_NOT_ENABLED;

  const cancel = () => {
    if (importedOnly) {
      onClose();
      return;
    }
    discard.mutate(initial.stagingId, { onSettled: onClose });
  };

  const importNow = () =>
    confirm.mutate(
      {
        stagingId: preview.stagingId,
        root: preview.root,
        name: trimmed,
        category,
        notes: notes.trim() === "" ? null : notes,
        enableNow,
      },
      { onSuccess: onClose },
    );

  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) cancel();
      }}
      dismissible={!busy}
      title={`Import ${preview.archiveName}`}
      description="Choose which archive folder corresponds to the game folder, then review where each file will go."
    >
      <div className={styles.stack}>
        <label className={styles.field}>
          <span>Install root</span>
          <select
            className={styles.input}
            value={preview.root ?? ""}
            disabled={busy || importedOnly}
            onChange={(e) =>
              remap.mutate(e.target.value === "" ? null : e.target.value, { onSuccess: setPreview })
            }
          >
            <option value="">(archive root)</option>
            {folders.map((f) => (
              <option key={f} value={f}>
                {f}
              </option>
            ))}
          </select>
        </label>
        {remap.error ? <ErrorMessage error={remap.error} /> : null}

        {nothingMapped ? (
          <p className={styles.muted}>
            No files of the archive map onto the game folder with this root. Choose another root
            folder.
          </p>
        ) : (
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Target</th>
                <th>Size</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {preview.mapped.map((m) => (
                <tr key={m.targetPath}>
                  <td className={styles.path}>{m.targetPath}</td>
                  <td className={styles.numeric}>{formatBytes(m.size)}</td>
                  <td>
                    <TargetBadge target={m} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {anyConflict ? (
          <p className={styles.muted}>
            Conflict: the file already exists in the game folder or belongs to an enabled mod.
          </p>
        ) : null}
        {anyProtected ? (
          <p role="alert">
            This archive would replace original game files, so it cannot be imported. Choose
            another root folder or use a different archive.
          </p>
        ) : null}

        <label className={styles.field}>
          <span>Name</span>
          <input className={styles.input} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <CategorySelect label="Category" value={category} onChange={setCategory} />
        <label className={styles.field}>
          <span>Notes</span>
          <textarea
            className={styles.input}
            rows={2}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </label>
        <label className={styles.checkRow}>
          <input
            type="checkbox"
            checked={enableNow}
            disabled={importedOnly}
            onChange={(e) => setEnableChoice(e.target.checked)}
          />
          <span>Enable after import</span>
        </label>

        {confirm.error ? <ErrorMessage error={confirm.error} /> : null}
        {discard.error ? <ErrorMessage error={discard.error} /> : null}
        <div className={styles.actions}>
          <Button onClick={cancel} disabled={busy}>
            {importedOnly ? "Close" : "Cancel"}
          </Button>
          {importedOnly ? null : (
            <Button
              variant="primary"
              loading={confirm.isPending}
              disabled={
                trimmed === "" ||
                nothingMapped ||
                anyProtected ||
                remap.isPending ||
                discard.isPending ||
                blockedReason !== null
              }
              title={blockedReason ?? undefined}
              onClick={importNow}
            >
              Import
            </Button>
          )}
        </div>
      </div>
    </Modal>
  );
}
