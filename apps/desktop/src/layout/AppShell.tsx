import {
  Activity,
  FolderCog,
  HardDrive,
  LayoutDashboard,
  type LucideIcon,
  Package,
  Settings,
  ShieldQuestion,
} from "lucide-react";
import { NavLink, Outlet } from "react-router";
import styles from "./AppShell.module.css";
import { InstallationSelector } from "./InstallationSelector";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
}

const NAV_ITEMS: readonly NavItem[] = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/mods", label: "Mods", icon: Package },
  { to: "/review", label: "Review", icon: ShieldQuestion },
  { to: "/profiles", label: "Profiles", icon: FolderCog },
  { to: "/activity", label: "Activity", icon: Activity },
  { to: "/installations", label: "Installations", icon: HardDrive },
  { to: "/settings", label: "Settings", icon: Settings },
];

/** Persistent story-mode notice (architecture §12). */
export const ONLINE_NOTICE =
  "Mods managed here are for story mode. Disabling mods does not make an installation safe for GTA Online.";

/** Application layout: fixed sidebar, routed content and the persistent notice. */
export function AppShell() {
  return (
    <div className={styles.shell}>
      <aside className={styles.sidebar}>
        <div className={styles.brand}>GTA V Mod Manager</div>
        <InstallationSelector />
        <nav aria-label="Main">
          <ul className={styles.navList}>
            {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
              <li key={to}>
                <NavLink
                  to={to}
                  end={to === "/"}
                  className={({ isActive }) =>
                    isActive ? `${styles.link} ${styles.active}` : styles.link
                  }
                >
                  <Icon size={16} aria-hidden="true" />
                  <span>{label}</span>
                </NavLink>
              </li>
            ))}
          </ul>
        </nav>
      </aside>
      <div className={styles.main}>
        <main className={styles.content}>
          <Outlet />
        </main>
        <div role="note" className={styles.notice}>
          {ONLINE_NOTICE}
        </div>
      </div>
    </div>
  );
}
