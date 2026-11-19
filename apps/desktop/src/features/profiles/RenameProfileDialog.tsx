import { useState } from "react";
import type { ProfileDto } from "../../bindings/ProfileDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Profiles.module.css";
import { useRenameProfile } from "./useProfiles";

/** Renames a stored profile. */
export function RenameProfileDialog({
  installationId,
  profile,
  onClose,
}: {
  installationId: string;
  profile: ProfileDto;
  onClose: () => void;
}) {
  const [name, setName] = useState(profile.name);
  const rename = useRenameProfile(installationId);
  const trimmed = name.trim();
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      dismissible={!rename.isPending}
      title={`Rename ${profile.name}`}
      description="Profile names are unique within the installation."
    >
      <form
        className={styles.stack}
        onSubmit={(ev) => {
          ev.preventDefault();
          rename.mutate({ profileId: profile.id, name: trimmed }, { onSuccess: onClose });
        }}
      >
        <label className={styles.field}>
          <span>New name</span>
          <input className={styles.input} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        {rename.error ? <ErrorMessage error={rename.error} /> : null}
        <div className={styles.actions}>
          <Button onClick={onClose} disabled={rename.isPending}>
            Cancel
          </Button>
          <Button
            type="submit"
            variant="primary"
            loading={rename.isPending}
            disabled={trimmed === "" || trimmed === profile.name}
          >
            Rename
          </Button>
        </div>
      </form>
    </Modal>
  );
}
