import { useState } from "react";
import { Link } from "react-router";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { formatDateTime } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import styles from "./Vanilla.module.css";
import { type VanillaAction, VanillaDialog } from "./VanillaDialog";
import { unmanagedNote } from "./labels";
import { useVanillaStatus } from "./useVanilla";

export interface VanillaPanelProps {
  installationId: string;
  /** Why writes are blocked right now, or `null`. */
  blockedReason: string | null;
}

/**
 * Vanilla mode: disable every managed mod in one step, then restore them. Renders nothing for
 * an installation without managed mods and without a restore point.
 */
export function VanillaPanel({ installationId, blockedReason }: VanillaPanelProps) {
  const status = useVanillaStatus(installationId);
  const [action, setAction] = useState<VanillaAction | null>(null);

  if (status.error) return <ErrorMessage error={status.error} />;
  const data = status.data;
  if (!data) return null;
  const restorePoint = data.restorePoint;
  if (data.modsTotal === 0 && !restorePoint) return null;
  const note = unmanagedNote(data.unmanagedFiles);

  return (
    <Card title="Vanilla mode">
      <div className={styles.stack}>
        <div className={styles.row}>
          {data.active ? (
            <Badge tone="info">Vanilla mode active</Badge>
          ) : (
            <span className={styles.muted}>
              {data.modsInGame} of {data.modsTotal} {data.modsTotal === 1 ? "mod" : "mods"} in
              the game folder
            </span>
          )}
          {restorePoint ? (
            <span className={styles.muted}>
              Restore point saved {formatDateTime(restorePoint.createdAt)} ·{" "}
              {restorePoint.entryCount} {restorePoint.entryCount === 1 ? "mod" : "mods"}
            </span>
          ) : null}
        </div>
        {data.active && note ? (
          <p className={styles.muted}>
            {note} <Link to="/review">Review</Link>
          </p>
        ) : null}
        <div className={styles.actions}>
          {data.modsInGame > 0 ? (
            <Button
              disabled={blockedReason !== null}
              title={blockedReason ?? undefined}
              onClick={() => setAction("enter")}
            >
              Disable all mods (vanilla)…
            </Button>
          ) : null}
          {restorePoint ? (
            <Button
              variant="primary"
              disabled={blockedReason !== null}
              title={blockedReason ?? undefined}
              onClick={() => setAction("restore")}
            >
              Restore my mods…
            </Button>
          ) : null}
        </div>
      </div>
      {action ? (
        <VanillaDialog
          installationId={installationId}
          action={action}
          status={data}
          blockedReason={blockedReason}
          onClose={() => setAction(null)}
        />
      ) : null}
    </Card>
  );
}
