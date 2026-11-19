import { useState } from "react";
import type { DesiredState } from "../../bindings/DesiredState";
import type { ProfileDetailDto } from "../../bindings/ProfileDetailDto";
import type { ProfileDto } from "../../bindings/ProfileDto";
import type { ProfileEntryInput } from "../../bindings/ProfileEntryInput";
import { Button } from "../../components/Button";
import { Modal } from "../../components/Modal";
import { ErrorMessage } from "../installations/ErrorMessage";
import { useMods } from "../mods/useMods";
import styles from "./Profiles.module.css";
import { useProfileDetail, useUpdateProfileEntries } from "./useProfiles";

export interface EditProfileDialogProps {
  installationId: string;
  profile: ProfileDto;
  onClose: () => void;
}

/** Edits which mods a profile wants enabled. It never touches the game folder. */
export function EditProfileDialog({ installationId, profile, onClose }: EditProfileDialogProps) {
  const detail = useProfileDetail(profile.id);
  return (
    <Modal
      open
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
      title={`Edit ${profile.name}`}
      description="Choose, for each mod, whether this profile enables it, disables it or leaves it alone. Saving does not change your game files; apply the profile to do that."
    >
      {detail.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {detail.error ? <ErrorMessage error={detail.error} /> : null}
      {detail.data ? (
        <EditBody installationId={installationId} detail={detail.data} onClose={onClose} />
      ) : null}
    </Modal>
  );
}

type Choice = DesiredState | "none";

const CHOICES: ReadonlyArray<{ value: Choice; label: string }> = [
  { value: "enabled", label: "Enabled" },
  { value: "disabled", label: "Disabled" },
  { value: "none", label: "Not in profile" },
];

function EditBody({
  installationId,
  detail,
  onClose,
}: {
  installationId: string;
  detail: ProfileDetailDto;
  onClose: () => void;
}) {
  const mods = useMods(installationId, true);
  const update = useUpdateProfileEntries(installationId, detail.id);
  const [edits, setEdits] = useState<Record<string, Choice>>({});

  if (mods.isPending) return <p className={styles.muted}>Loading…</p>;
  if (mods.error) return <ErrorMessage error={mods.error} />;

  // Every mod of the installation, plus any profile entry whose mod is no longer listed.
  const names = new Map<string, string>();
  for (const e of detail.entries) names.set(e.modId, e.modName);
  for (const m of mods.data) names.set(m.id, m.name);
  const rows = [...names.entries()]
    .map(([modId, modName]) => ({ modId, modName }))
    .sort((a, b) => a.modName.localeCompare(b.modName));
  const saved = new Map<string, Choice>(detail.entries.map((e) => [e.modId, e.desired]));
  const current = (modId: string): Choice => edits[modId] ?? saved.get(modId) ?? "none";
  const changed = rows.some((r) => current(r.modId) !== (saved.get(r.modId) ?? "none"));

  const save = () => {
    const entries: ProfileEntryInput[] = [];
    for (const r of rows) {
      const choice = current(r.modId);
      if (choice !== "none") entries.push({ modId: r.modId, desired: choice });
    }
    update.mutate(entries, { onSuccess: onClose });
  };

  return (
    <div className={styles.stack}>
      {rows.length === 0 ? (
        <p className={styles.muted}>There are no managed mods in this installation yet.</p>
      ) : (
        <>
          <p className={styles.muted}>
            Mods marked "Not in profile" are left unchanged when the profile is applied.
          </p>
          <table className={styles.table}>
            <thead>
              <tr>
                <th>Mod</th>
                <th>In this profile</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.modId}>
                  <td>{r.modName}</td>
                  <td>
                    <select
                      className={styles.input}
                      aria-label={`${r.modName} in this profile`}
                      value={current(r.modId)}
                      onChange={(ev) => {
                        const next = CHOICES.find((c) => c.value === ev.target.value);
                        if (next) setEdits({ ...edits, [r.modId]: next.value });
                      }}
                    >
                      {CHOICES.map((c) => (
                        <option key={c.value} value={c.value}>
                          {c.label}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
      {update.error ? <ErrorMessage error={update.error} /> : null}
      <div className={styles.actions}>
        <Button onClick={onClose} disabled={update.isPending}>
          Cancel
        </Button>
        <Button variant="primary" loading={update.isPending} disabled={!changed} onClick={save}>
          Save
        </Button>
      </div>
    </div>
  );
}
