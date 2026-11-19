import { SettingsView } from "../features/settings/SettingsView";
import styles from "./Page.module.css";

/** Settings page: read-only application information. */
export function SettingsPage() {
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Settings</h1>
      <SettingsView />
    </div>
  );
}
