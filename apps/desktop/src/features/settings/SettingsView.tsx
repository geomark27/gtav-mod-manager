import type { ReactNode } from "react";
import { Badge } from "../../components/Badge";
import { Card } from "../../components/Card";
import { formatDateTime } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import { UpdatesCard } from "../updates/UpdatesCard";
import { VaultCard } from "../vault/VaultCard";
import { useAppStatus } from "../status/useAppStatus";
import styles from "./Settings.module.css";

/** Shown on Settings; the same rule the sidebar note states. */
export const STORY_MODE_NOTICE =
  "Mods managed here are for story mode. Disabling mods does not make an installation safe for GTA Online.";

function Row({ label, mono = false, children }: { label: string; mono?: boolean; children: ReactNode }) {
  return (
    <>
      <dt className={styles.label}>{label}</dt>
      <dd className={mono ? `${styles.value} ${styles.mono}` : styles.value}>{children}</dd>
    </>
  );
}

/** Read-only information about the application. */
export function SettingsView() {
  const status = useAppStatus();
  if (status.isPending) return <p className={styles.muted}>Loading…</p>;
  if (status.error) return <ErrorMessage error={status.error} />;
  const data = status.data;
  const r = data.recovery;
  return (
    <div className={styles.stack}>
      <Card title="About">
        <dl className={styles.rows}>
          <Row label="App version">{data.appVersion}</Row>
          <Row label="Started">{formatDateTime(data.startedAt)}</Row>
        </dl>
      </Card>
      <Card title="Locations">
        <dl className={styles.rows}>
          <Row label="Data folder" mono>
            {data.dataDir}
          </Row>
          <Row label="Vault folder" mono>
            {data.vaultDir}
          </Row>
          <Row label="Logs folder" mono>
            {data.logsDir}
          </Row>
          <Row label="Database" mono>
            {data.dbPath}
          </Row>
        </dl>
      </Card>
      <Card title="Startup recovery">
        <p className={styles.muted}>What the app did with interrupted changes when it started.</p>
        <dl className={styles.rows}>
          <Row label="Undone">{r.rolledBack}</Row>
          <Row label="Finished">{r.committed}</Row>
          <Row label="Needs attention">
            {r.needsAttention > 0 ? <Badge tone="danger">{r.needsAttention}</Badge> : r.needsAttention}
          </Row>
          <Row label="Skipped">{r.skipped}</Row>
        </dl>
      </Card>
      <VaultCard />
      <UpdatesCard />
      <p>{STORY_MODE_NOTICE}</p>
    </div>
  );
}
