import { Route, Routes } from "react-router";
import { AppShell } from "./layout/AppShell";
import { ActivityPage } from "./pages/ActivityPage";
import { DashboardPage } from "./pages/DashboardPage";
import { InstallationsPage } from "./pages/InstallationsPage";
import { ModsPage } from "./pages/ModsPage";
import { NotFoundPage } from "./pages/NotFoundPage";
import { ProfilesPage } from "./pages/ProfilesPage";
import { ReviewPage } from "./pages/ReviewPage";
import { SettingsPage } from "./pages/SettingsPage";

/** Route table. Must be rendered inside a router. */
export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<DashboardPage />} />
        <Route path="mods" element={<ModsPage />} />
        <Route path="review" element={<ReviewPage />} />
        <Route path="profiles" element={<ProfilesPage />} />
        <Route path="activity" element={<ActivityPage />} />
        <Route path="installations" element={<InstallationsPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<NotFoundPage />} />
      </Route>
    </Routes>
  );
}
