import { useState } from "react";
import type { InstallationDto } from "../../bindings/InstallationDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { ErrorMessage } from "./ErrorMessage";
import styles from "./Installations.module.css";
import { EDITION_LABEL, PLATFORM_LABEL, READ_ONLY_NOTICE } from "./labels";
import { StatusBadge } from "./StatusBadge";
import {
  useRefreshInstallation,
  useRenameInstallation,
  useSelectInstallation,
} from "./useInstallations";

export interface InstallationCardProps {
  installation: InstallationDto;
  isActive: boolean;
}

/** One registered installation with refresh, rename and set-active actions. */
export function InstallationCard({ installation, isActive }: InstallationCardProps) {
  const refresh = useRefreshInstallation();
  const select = useSelectInstallation();
  const rename = useRenameInstallation();
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(installation.displayName);

  const error = refresh.error ?? select.error ?? rename.error;

  const startEdit = () => {
    setName(installation.displayName);
    rename.reset();
    setEditing(true);
  };

  const save = () => {
    rename.mutate(
      { id: installation.id, displayName: name },
      { onSuccess: () => setEditing(false) },
    );
  };

  return (
    <Card>
      <div className={styles.cardHeader}>
        {editing ? (
          <form
            className={styles.renameForm}
            onSubmit={(e) => {
              e.preventDefault();
              save();
            }}
          >
            <input
              className={styles.input}
              aria-label="Installation name"
              value={name}
              maxLength={80}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
            <Button type="submit" variant="primary" loading={rename.isPending}>
              Save
            </Button>
            <Button onClick={() => setEditing(false)} disabled={rename.isPending}>
              Cancel
            </Button>
          </form>
        ) : (
          <>
            <h2 className={styles.name}>{installation.displayName}</h2>
            <Button variant="ghost" onClick={startEdit}>
              Rename
            </Button>
          </>
        )}
      </div>
      <div className={styles.badges}>
        <Badge tone="info">{EDITION_LABEL[installation.edition]}</Badge>
        <Badge tone="neutral">{PLATFORM_LABEL[installation.platform]}</Badge>
        <StatusBadge status={installation.status} />
        {isActive ? <Badge tone="success">Active</Badge> : null}
      </div>
      <p className={styles.path}>{installation.rootPath}</p>
      <p className={styles.muted}>{installation.exeVersion ?? "Unknown version"}</p>
      {installation.status === "not_writable" ? (
        <p className={styles.notice}>{READ_ONLY_NOTICE}</p>
      ) : null}
      {error ? <ErrorMessage error={error} /> : null}
      <div className={styles.actions}>
        <Button
          onClick={() => refresh.mutate(installation.id)}
          loading={refresh.isPending}
          aria-label={`Refresh ${installation.displayName}`}
        >
          Refresh
        </Button>
        {isActive ? null : (
          <Button
            variant="primary"
            onClick={() => select.mutate(installation.id)}
            loading={select.isPending}
            aria-label={`Set ${installation.displayName} as active`}
          >
            Set active
          </Button>
        )}
      </div>
    </Card>
  );
}
