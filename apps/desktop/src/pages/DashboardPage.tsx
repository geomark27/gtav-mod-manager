import { Link } from "react-router";
import { DashboardView } from "../features/dashboard/DashboardView";
import { ErrorMessage } from "../features/installations/ErrorMessage";
import { useSelectedInstallation } from "../features/installations/useInstallations";
import { StatusCard } from "../features/status/StatusCard";
import styles from "./Page.module.css";

/** Dashboard page for the active installation. */
export function DashboardPage() {
  const selected = useSelectedInstallation();
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Dashboard</h1>
      {selected.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {selected.error ? <ErrorMessage error={selected.error} /> : null}
      {selected.data ? (
        <DashboardView key={selected.data.id} installationId={selected.data.id} />
      ) : selected.data === null ? (
        <p className={styles.muted}>
          No active installation. <Link to="/installations">Go to Installations</Link> to add or
          select one.
        </p>
      ) : null}
      <StatusCard />
    </div>
  );
}
