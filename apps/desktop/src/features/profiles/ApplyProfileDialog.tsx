import type { ProfileDto } from "../../bindings/ProfileDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { formatBytes } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Profiles.module.css";
import { useApplyProfile, usePreviewApplyProfile } from "./useProfiles";

export interface ApplyProfileDialogProps {
  installationId: string;
  profile: ProfileDto;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
  onClose: () => void;
}

/** Dry run of a profile with the mods it would change; Apply runs it. */
export function ApplyProfileDialog({
  installationId,
  profile,
  blockedReason,
  onClose,
}: ApplyProfileDialogProps) {
  const preview = usePreviewApplyProfile(installationId, profile.id);
  const apply = useApplyProfile(installationId);
  const data = preview.data;
  const nothing = data !== undefined && data.modsAffected.length === 0;
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      dismissible={!apply.isPending}
      title={`Apply ${profile.name}`}
      description="Review what will change in the game folder before applying."
    >
      {preview.isPending ? <p className={styles.muted}>Calculating changes…</p> : null}
      {preview.error ? <ErrorMessage error={preview.error} /> : null}
      {data ? (
        <div className={styles.stack}>
          {nothing ? (
            <p>Nothing to change: the mods already match this profile.</p>
          ) : (
            <>
              <p>
                {data.modsAffected.length} {data.modsAffected.length === 1 ? "mod" : "mods"} affected
                · {formatBytes(data.bytesToCopy)} to copy
              </p>
              <ul className={styles.plainList}>
                {data.modsAffected.map((m) => (
                  <li key={m.modId}>
                    {m.name}: {m.from} → {m.to}
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      ) : null}
      {apply.error ? <ErrorMessage error={apply.error} /> : null}
      <div className={styles.actions}>
        <Button onClick={onClose} disabled={apply.isPending}>
          Cancel
        </Button>
        <Button
          variant="primary"
          loading={apply.isPending}
          disabled={!data || nothing || blockedReason !== null}
          title={blockedReason ?? undefined}
          onClick={() => apply.mutate(profile.id, { onSuccess: onClose })}
        >
          Apply
        </Button>
      </div>
    </Modal>
  );
}
