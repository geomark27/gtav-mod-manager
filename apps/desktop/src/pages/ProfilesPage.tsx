import { Link } from "react-router";
import { ProfilesView } from "../features/profiles/ProfilesView";
import { ErrorMessage } from "../features/installations/ErrorMessage";
import { useSelectedInstallation } from "../features/installations/useInstallations";
import styles from "./Page.module.css";

/** Profiles page for the active installation. */
export function ProfilesPage() {
  const selected = useSelectedInstallation();
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Profiles</h1>
      {selected.isPending ? <p className={styles.muted}>Loading…</p> : null}
      {selected.error ? <ErrorMessage error={selected.error} /> : null}
      {selected.data ? (
        <ProfilesView key={selected.data.id} installationId={selected.data.id} />
      ) : selected.data === null ? (
        <p className={styles.muted}>
          No active installation. <Link to="/installations">Go to Installations</Link> to add or
          select one.
        </p>
      ) : null}
    </div>
  );
}
