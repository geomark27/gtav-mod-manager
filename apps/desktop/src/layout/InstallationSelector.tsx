import { Link } from "react-router";
import {
  useInstallations,
  useSelectedInstallation,
  useSelectInstallation,
} from "../features/installations/useInstallations";
import styles from "./AppShell.module.css";

/** Sidebar selector of the active installation. */
export function InstallationSelector() {
  const installations = useInstallations();
  const selected = useSelectedInstallation();
  const select = useSelectInstallation();

  let body: React.ReactNode;
  if (installations.isPending) {
    body = <p className={styles.selectorText}>Loading…</p>;
  } else if (installations.error) {
    body = <p className={styles.selectorText}>Installations unavailable</p>;
  } else if (installations.data.length === 0) {
    body = (
      <Link to="/installations" className={styles.selectorLink}>
        No installation
      </Link>
    );
  } else {
    body = (
      <select
        id="active-installation"
        className={styles.select}
        value={selected.data?.id ?? ""}
        disabled={select.isPending}
        onChange={(e) => select.mutate(e.target.value)}
      >
        {selected.data ? null : (
          <option value="" disabled>
            Select installation
          </option>
        )}
        {installations.data.map((i) => (
          <option key={i.id} value={i.id}>
            {i.displayName}
          </option>
        ))}
      </select>
    );
  }

  return (
    <div className={styles.selector}>
      <label htmlFor="active-installation" className={styles.selectorLabel}>
        Active installation
      </label>
      {body}
    </div>
  );
}
