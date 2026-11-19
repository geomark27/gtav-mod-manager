import { Badge } from "../../components/Badge";
import { Card } from "../../components/Card";
import styles from "./StatusCard.module.css";
import { useAppStatus } from "./useAppStatus";

interface RowProps {
  label: string;
  children: React.ReactNode;
  mono?: boolean;
}

function Row({ label, children, mono = false }: RowProps) {
  return (
    <>
      <dt className={styles.label}>{label}</dt>
      <dd className={mono ? `${styles.value} ${styles.mono}` : styles.value}>{children}</dd>
    </>
  );
}

/** Dashboard card with real backend status data. */
export function StatusCard() {
  const { data, error, isPending } = useAppStatus();

  if (isPending) {
    return (
      <Card title="System status">
        <p className={styles.message}>Loading status…</p>
      </Card>
    );
  }

  if (error) {
    return (
      <Card title="System status">
        <div className={styles.error} role="alert">
          <Badge tone="danger">{error.code}</Badge>
          <p className={styles.message}>{error.message}</p>
          {error.hint ? <p className={styles.hint}>{error.hint}</p> : null}
        </div>
      </Card>
    );
  }

  return (
    <Card title="System status">
      <dl className={styles.rows}>
        <Row label="App version">{data.appVersion}</Row>
        <Row label="Schema version">{data.schemaVersion}</Row>
        <Row label="Journal mode">{data.journalMode}</Row>
        <Row label="Synchronous">{data.synchronous}</Row>
        <Row label="Foreign keys">
          <Badge tone={data.foreignKeys ? "success" : "danger"}>
            {data.foreignKeys ? "On" : "Off"}
          </Badge>
        </Row>
        <Row label="Data folder" mono>
          {data.dataDir}
        </Row>
        <Row label="Vault folder" mono>
          {data.vaultDir}
        </Row>
        <Row label="Logs folder" mono>
          {data.logsDir}
        </Row>
      </dl>
    </Card>
  );
}
