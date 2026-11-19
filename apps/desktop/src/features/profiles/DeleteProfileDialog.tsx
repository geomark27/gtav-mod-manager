import type { ProfileDto } from "../../bindings/ProfileDto";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Profiles.module.css";
import { useDeleteProfile } from "./useProfiles";

/** Confirms deleting a profile. Mods and game files are not affected. */
export function DeleteProfileDialog({
  installationId,
  profile,
  onClose,
}: {
  installationId: string;
  profile: ProfileDto;
  onClose: () => void;
}) {
  const remove = useDeleteProfile(installationId);
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      dismissible={!remove.isPending}
      title={`Delete ${profile.name}`}
      description="Deleting a profile does not change your mods or game files."
    >
      <div className={styles.stack}>
        <p>Delete the profile “{profile.name}”? This cannot be undone.</p>
        {remove.error ? <ErrorMessage error={remove.error} /> : null}
        <div className={styles.actions}>
          <Button onClick={onClose} disabled={remove.isPending}>
            Cancel
          </Button>
          <Button
            variant="danger"
            loading={remove.isPending}
            onClick={() => remove.mutate(profile.id, { onSuccess: onClose })}
          >
            Delete profile
          </Button>
        </div>
      </div>
    </Modal>
  );
}
