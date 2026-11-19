import { AddManualButton } from "../features/installations/AddManualButton";
import { DiscoverPanel } from "../features/installations/DiscoverPanel";
import { InstallationCard } from "../features/installations/InstallationCard";
import { ErrorMessage } from "../features/installations/ErrorMessage";
import installStyles from "../features/installations/Installations.module.css";
import {
  useInstallations,
  useSelectedInstallation,
} from "../features/installations/useInstallations";
import styles from "./Page.module.css";

/** Installations page: detect, add, refresh, rename and choose the active installation. */
export function InstallationsPage() {
  const installations = useInstallations();
  const selected = useSelectedInstallation();

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Installations</h1>
      <AddManualButton />
      <DiscoverPanel />
      <section aria-label="Registered installations" className={styles.section}>
        {installations.error ? <ErrorMessage error={installations.error} /> : null}
        {installations.data?.length === 0 ? (
          <p className={styles.muted}>No installation registered yet.</p>
        ) : null}
        <div className={installStyles.list}>
          {installations.data?.map((i) => (
            <InstallationCard key={i.id} installation={i} isActive={selected.data?.id === i.id} />
          ))}
        </div>
      </section>
    </div>
  );
}
