import { useState } from "react";
import type { Category } from "../../bindings/Category";
import type { ModDetailDto } from "../../bindings/ModDetailDto";
import type { ModDto } from "../../bindings/ModDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { formatBytes } from "../../lib/format";
import { DependenciesSection } from "../dependencies/DependenciesSection";
import { ErrorMessage } from "../installations/ErrorMessage";
import { CategorySelect } from "../inventory/CategorySelect";
import { sourceLabel } from "./modLabels";
import styles from "./Mods.module.css";
import { useForgetMod, useModDetail, useReadoptMod, useRestoreMod, useUpdateMod } from "./useMods";

export interface ModDetailsDialogProps {
  installationId: string;
  mod: ModDto;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
  onClose: () => void;
}

/** Details panel of one mod: files, edit form and "Stop managing". */
export function ModDetailsDialog({
  installationId,
  mod,
  blockedReason,
  onClose,
}: ModDetailsDialogProps) {
  const detail = useModDetail(mod.id);
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      title={mod.name}
      description="Files, name, category and notes of this mod."
    >
      {detail.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {detail.error ? <ErrorMessage error={detail.error} /> : null}
      {detail.data ? (
        <DetailsBody
          installationId={installationId}
          mod={mod}
          detail={detail.data}
          blockedReason={blockedReason}
          onClose={onClose}
        />
      ) : null}
    </Modal>
  );
}

interface BodyProps extends ModDetailsDialogProps {
  detail: ModDetailDto;
}

function DetailsBody({ installationId, mod, detail, blockedReason, onClose }: BodyProps) {
  const [name, setName] = useState(detail.name);
  const [category, setCategory] = useState<Category>(detail.category);
  const [notes, setNotes] = useState(detail.notes ?? "");
  const [confirming, setConfirming] = useState(false);
  const [resolving, setResolving] = useState<"restore" | "keep" | null>(null);
  const update = useUpdateMod(installationId, mod.id);
  const forget = useForgetMod(installationId);
  const restore = useRestoreMod(installationId);
  const readopt = useReadoptMod(installationId);

  const resolveError = resolving === "restore" ? restore.error : readopt.error;
  const trimmed = name.trim();
  const newNotes = notes.trim() === "" ? null : notes;
  const changed =
    trimmed !== detail.name || category !== detail.category || newNotes !== detail.notes;
  const hasVersions = detail.entries.some((e) => e.fileVersion !== null);
  const canStop = mod.state === "enabled" || mod.state === "drifted" || mod.state === "error";

  return (
    <div className={styles.stack}>
      <p className={styles.muted} title={detail.source.sha256 ?? undefined}>
        Source: {sourceLabel(detail.source, detail.createdAt)}
      </p>
      <table className={styles.table}>
        <thead>
          <tr>
            <th>Path</th>
            <th>Size</th>
            <th>Hash</th>
            {hasVersions ? <th>Version</th> : null}
          </tr>
        </thead>
        <tbody>
          {detail.entries.map((e) => (
            <tr key={e.relPath}>
              <td className={styles.path}>{e.relPath}</td>
              <td className={styles.numeric}>{formatBytes(e.size)}</td>
              <td className={styles.path}>{e.sha256.slice(0, 8)}</td>
              {hasVersions ? <td>{e.fileVersion ?? ""}</td> : null}
            </tr>
          ))}
        </tbody>
      </table>

      <DependenciesSection installationId={installationId} detail={detail} />

      <form
        className={styles.stack}
        onSubmit={(ev) => {
          ev.preventDefault();
          update.mutate({ name: trimmed, category, notes: newNotes });
        }}
      >
        <label className={styles.field}>
          <span>Name</span>
          <input className={styles.input} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <CategorySelect label="Category" value={category} onChange={setCategory} />
        <label className={styles.field}>
          <span>Notes</span>
          <textarea
            className={styles.input}
            rows={3}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </label>
        {update.error ? <ErrorMessage error={update.error} /> : null}
        <div className={styles.actions}>
          <Button
            type="submit"
            variant="primary"
            loading={update.isPending}
            disabled={!changed || trimmed === ""}
          >
            Save
          </Button>
        </div>
      </form>

      {mod.state === "drifted" ? (
        <div className={styles.stack}>
          <p>
            The files of this mod in the game folder no longer match the managed version.
          </p>
          {resolving === null ? (
            <div className={styles.actions}>
              <Button
                disabled={blockedReason !== null}
                title={blockedReason ?? undefined}
                onClick={() => setResolving("restore")}
              >
                Restore vault version
              </Button>
              <Button
                disabled={blockedReason !== null}
                title={blockedReason ?? undefined}
                onClick={() => setResolving("keep")}
              >
                Keep current files
              </Button>
            </div>
          ) : (
            <>
              <p>
                {resolving === "restore"
                  ? "Your current files are kept in the vault, then the managed version is put back."
                  : "The current files become the managed version."}
              </p>
              {resolveError ? <ErrorMessage error={resolveError} /> : null}
              <div className={styles.actions}>
                <Button
                  onClick={() => setResolving(null)}
                  disabled={restore.isPending || readopt.isPending}
                >
                  Cancel
                </Button>
                <Button
                  variant="primary"
                  loading={resolving === "restore" ? restore.isPending : readopt.isPending}
                  onClick={() =>
                    (resolving === "restore" ? restore : readopt).mutate(mod.id, {
                      onSuccess: onClose,
                    })
                  }
                >
                  {resolving === "restore" ? "Confirm restore" : "Confirm keep current files"}
                </Button>
              </div>
            </>
          )}
        </div>
      ) : null}

      <div className={styles.stack}>
        {canStop ? (
          confirming ? (
            <>
              <p>
                The files stay in the game folder; the app stops tracking them. The vault copy is
                kept.
              </p>
              {forget.error ? <ErrorMessage error={forget.error} /> : null}
              <div className={styles.actions}>
                <Button onClick={() => setConfirming(false)} disabled={forget.isPending}>
                  Keep managing
                </Button>
                <Button
                  variant="danger"
                  loading={forget.isPending}
                  onClick={() => forget.mutate(mod.id, { onSuccess: onClose })}
                >
                  Confirm stop managing
                </Button>
              </div>
            </>
          ) : (
            <div className={styles.actions}>
              <Button
                variant="danger"
                disabled={blockedReason !== null}
                title={blockedReason ?? undefined}
                onClick={() => setConfirming(true)}
              >
                Stop managing
              </Button>
            </div>
          )
        ) : (
          <p className={styles.muted}>
            Only enabled, drifted or error mods can stop being managed.
          </p>
        )}
      </div>
    </div>
  );
}
