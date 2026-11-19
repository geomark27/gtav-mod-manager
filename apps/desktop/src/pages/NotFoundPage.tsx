import styles from "./Page.module.css";

/** Fallback for unknown routes. */
export function NotFoundPage() {
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Page not found</h1>
      <p className={styles.muted}>This page does not exist.</p>
    </div>
  );
}
