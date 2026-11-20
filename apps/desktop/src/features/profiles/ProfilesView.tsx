import { useState } from "react";
import type { ProfileDto } from "../../bindings/ProfileDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { GuardBanner } from "../guards/GuardBanner";
import { guardNotice } from "../guards/blocking";
import { useGuardStatus } from "../guards/useGuardStatus";
import { ErrorMessage } from "../installations/ErrorMessage";
import { ApplyProfileDialog } from "./ApplyProfileDialog";
import { DeleteProfileDialog } from "./DeleteProfileDialog";
import { EditProfileDialog } from "./EditProfileDialog";
import styles from "./Profiles.module.css";
import { RenameProfileDialog } from "./RenameProfileDialog";
import { SaveProfileDialog } from "./SaveProfileDialog";
import { useProfiles } from "./useProfiles";

type Dialog =
  | { kind: "save" }
  | { kind: "apply" | "edit" | "rename" | "delete"; profile: ProfileDto };

/** Profiles of one installation: save, preview and apply, edit, rename, delete. */
export function ProfilesView({ installationId }: { installationId: string }) {
  const profiles = useProfiles(installationId);
  const guard = useGuardStatus(installationId);
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const close = () => setDialog(null);

  const notice = guard.data ? guardNotice(guard.data) : null;
  // While the first guard answer is pending, applying stays blocked (like the Mods switches).
  const blockedReason = guard.data ? (notice?.text ?? null) : guard.error ? null : "Checking whether the game is running…";

  return (
    <div className={styles.stack}>
      {notice ? <GuardBanner notice={notice} /> : null}
      {guard.error ? <ErrorMessage error={guard.error} /> : null}
      <div className={styles.toolbar}>
        <Button variant="primary" onClick={() => setDialog({ kind: "save" })}>
          Save current state as profile…
        </Button>
      </div>
      {profiles.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {profiles.error ? <ErrorMessage error={profiles.error} /> : null}
      {profiles.data ? (
        <ul className={styles.list}>
          {profiles.data.map((p) => (
            <li key={p.id} className={styles.item} aria-label={p.name}>
              <span className={styles.name}>
                {p.name}
                {p.builtIn ? <Badge tone="neutral">Built-in</Badge> : null}
                {p.kind === "restore_point" ? <Badge tone="info">Restore point</Badge> : null}
                <span className={styles.muted}>
                  {p.entryCount} {p.entryCount === 1 ? "mod" : "mods"}
                </span>
              </span>
              <span className={styles.actions}>
                <Button onClick={() => setDialog({ kind: "apply", profile: p })}>
                  Preview &amp; apply
                </Button>
                {!p.builtIn ? (
                  <>
                    <Button onClick={() => setDialog({ kind: "edit", profile: p })}>Edit</Button>
                    <Button onClick={() => setDialog({ kind: "rename", profile: p })}>Rename</Button>
                    <Button variant="danger" onClick={() => setDialog({ kind: "delete", profile: p })}>
                      Delete
                    </Button>
                  </>
                ) : null}
              </span>
            </li>
          ))}
        </ul>
      ) : null}

      {dialog?.kind === "save" ? (
        <SaveProfileDialog installationId={installationId} onClose={close} />
      ) : null}
      {dialog?.kind === "apply" ? (
        <ApplyProfileDialog
          installationId={installationId}
          profile={dialog.profile}
          blockedReason={blockedReason}
          onClose={close}
        />
      ) : null}
      {dialog?.kind === "edit" ? (
        <EditProfileDialog installationId={installationId} profile={dialog.profile} onClose={close} />
      ) : null}
      {dialog?.kind === "rename" ? (
        <RenameProfileDialog
          installationId={installationId}
          profile={dialog.profile}
          onClose={close}
        />
      ) : null}
      {dialog?.kind === "delete" ? (
        <DeleteProfileDialog
          installationId={installationId}
          profile={dialog.profile}
          onClose={close}
        />
      ) : null}
    </div>
  );
}
