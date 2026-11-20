import type { VanillaStatusDto } from "../../bindings/VanillaStatusDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { VANILLA_PROFILE_ID } from "../../lib/ipc";
import { formatBytes } from "../../lib/format";
import { ONLINE_NOTICE } from "../../layout/AppShell";
import { ErrorMessage } from "../installations/ErrorMessage";
import { usePreviewApplyProfile } from "../profiles/useProfiles";
import styles from "./Vanilla.module.css";
import { useEnterVanilla, useRestoreFromVanilla } from "./useVanilla";
import { unmanagedNote } from "./labels";

export type VanillaAction = "enter" | "restore";

export interface VanillaDialogProps {
  installationId: string;
  action: VanillaAction;
  status: VanillaStatusDto;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
  onClose: () => void;
}

/** Preview and confirmation of vanilla mode or of restoring the mods. */
export function VanillaDialog({
  installationId,
  action,
  status,
  blockedReason,
  onClose,
}: VanillaDialogProps) {
  const enter = action === "enter";
  const profileId = enter ? VANILLA_PROFILE_ID : (status.restorePoint?.id ?? "");
  const preview = usePreviewApplyProfile(installationId, profileId);
  const enterVanilla = useEnterVanilla(installationId);
  const restore = useRestoreFromVanilla(installationId);
  const mutation = enter ? enterVanilla : restore;
  const data = preview.data;
  const note = enter ? unmanagedNote(status.unmanagedFiles) : null;

  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      dismissible={!mutation.isPending}
      title={enter ? "Disable all mods (vanilla)" : "Restore my mods"}
      description={
        enter
          ? "Your current mod states are saved as a restore point, then every managed mod is disabled in one step."
          : `The states saved in “${status.restorePoint?.name ?? ""}” are applied in one step.`
      }
    >
      <div className={styles.stack}>
        {preview.isPending ? <p className={styles.muted}>Calculating changes…</p> : null}
        {preview.error ? <ErrorMessage error={preview.error} /> : null}
        {data ? (
          data.modsAffected.length === 0 ? (
            <p>Nothing to change in the game folder.</p>
          ) : (
            <>
              <p>
                {data.modsAffected.length} {data.modsAffected.length === 1 ? "mod" : "mods"}{" "}
                affected · {formatBytes(data.bytesToCopy)} to copy
              </p>
              <ul className={styles.plainList}>
                {data.modsAffected.map((m) => (
                  <li key={m.modId}>
                    {m.name}: {m.from} → {m.to}
                  </li>
                ))}
              </ul>
            </>
          )
        ) : null}
        {note ? <p className={styles.muted}>{note}</p> : null}
        <p className={styles.notice} role="note">
          {ONLINE_NOTICE}
        </p>
        {mutation.error ? <ErrorMessage error={mutation.error} /> : null}
        <div className={styles.actions}>
          <Button onClick={onClose} disabled={mutation.isPending}>
            Cancel
          </Button>
          <Button
            variant="primary"
            loading={mutation.isPending}
            disabled={!data || blockedReason !== null}
            title={blockedReason ?? undefined}
            onClick={() => mutation.mutate(undefined, { onSuccess: onClose })}
          >
            {enter ? "Disable all mods" : "Restore my mods"}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
