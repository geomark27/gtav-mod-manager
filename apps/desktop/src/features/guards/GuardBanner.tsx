import type { GuardNotice } from "./blocking";
import styles from "./Guards.module.css";

/** Banner explaining why mods cannot be changed. */
export function GuardBanner({ notice }: { notice: GuardNotice }) {
  return (
    <div
      className={`${styles.banner} ${styles[notice.severity]}`}
      role={notice.severity === "danger" ? "alert" : "status"}
    >
      {notice.text}
    </div>
  );
}
