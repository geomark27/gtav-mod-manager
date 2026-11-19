import { Link } from "react-router";
import { ActivityView } from "../features/activity/ActivityView";
import { ErrorMessage } from "../features/installations/ErrorMessage";
import { useSelectedInstallation } from "../features/installations/useInstallations";
import styles from "./Page.module.css";

/** Activity page for the active installation. */
export function ActivityPage() {
  const selected = useSelectedInstallation();
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Activity</h1>
      {selected.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {selected.error ? <ErrorMessage error={selected.error} /> : null}
      {selected.data ? (
        <ActivityView key={selected.data.id} installationId={selected.data.id} />
      ) : selected.data === null ? (
        <p className={styles.muted}>
          No active installation. <Link to="/installations">Go to Installations</Link> to add or
          select one.
        </p>
      ) : null}
    </div>
  );
}
