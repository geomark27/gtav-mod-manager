import { Link } from "react-router";
import { ErrorMessage } from "../features/installations/ErrorMessage";
import { useSelectedInstallation } from "../features/installations/useInstallations";
import { ModsView } from "../features/mods/ModsView";
import styles from "./Page.module.css";

/** Mods page: managed mods with verified switches, grouped by category. */
export function ModsPage() {
  const selected = useSelectedInstallation();
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Mods</h1>
      {selected.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {selected.error ? <ErrorMessage error={selected.error} /> : null}
      {selected.data ? (
        <ModsView
          key={selected.data.id}
          installationId={selected.data.id}
          name={selected.data.displayName}
        />
      ) : selected.data === null ? (
        <p className={styles.muted}>
          No active installation. <Link to="/installations">Go to Installations</Link> to add or
          select one.
        </p>
      ) : null}
    </div>
  );
}
