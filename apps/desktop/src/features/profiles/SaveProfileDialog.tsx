import { useState } from "react";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Profiles.module.css";
import { useCreateProfile, useProfileNameSuggestions } from "./useProfiles";

/** Saves the current enabled/disabled state of every mod as a new profile. */
export function SaveProfileDialog({
  installationId,
  onClose,
}: {
  installationId: string;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  const create = useCreateProfile(installationId);
  const suggestions = useProfileNameSuggestions();
  const trimmed = name.trim();
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      dismissible={!create.isPending}
      title="Save current state as profile"
      description="The profile remembers which mods are enabled and disabled right now."
    >
      <form
        className={styles.stack}
        onSubmit={(ev) => {
          ev.preventDefault();
          create.mutate(trimmed, { onSuccess: onClose });
        }}
      >
        <label className={styles.field}>
          <span>Profile name</span>
          <input className={styles.input} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        {suggestions.data && suggestions.data.length > 0 ? (
          <div className={styles.chips} aria-label="Name suggestions" role="group">
            {suggestions.data.map((s) => (
              <Button key={s} variant="ghost" onClick={() => setName(s)}>
                {s}
              </Button>
            ))}
          </div>
        ) : null}
        {create.error ? <ErrorMessage error={create.error} /> : null}
        <div className={styles.actions}>
          <Button onClick={onClose} disabled={create.isPending}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" loading={create.isPending} disabled={trimmed === ""}>
            Save profile
          </Button>
        </div>
      </form>
    </Modal>
  );
}
