import * as Tooltip from "@radix-ui/react-tooltip";
import { useState } from "react";
import { Link } from "react-router";
import type { ModDto } from "../../bindings/ModDto";
import { GuardBanner } from "../guards/GuardBanner";
import { guardNotice } from "../guards/blocking";
import { useGuardStatus } from "../guards/useGuardStatus";
import { ImportZipButton } from "../import/ImportZip";
import { ErrorMessage } from "../installations/ErrorMessage";
import { VanillaPanel } from "../vanilla/VanillaPanel";
import { CATEGORIES } from "../inventory/labels";
import { ModDetailsDialog } from "./ModDetailsDialog";
import { ModGroup } from "./ModGroup";
import { NO_FILTERS, ModsToolbar, matchesFilters } from "./ModsToolbar";
import type { ModFilters } from "./ModsToolbar";
import styles from "./Mods.module.css";
import { useMods, useReconcile } from "./useMods";

/** Mods of one installation. */
export function ModsView({ installationId, name }: { installationId: string; name: string }) {
  const [filters, setFilters] = useState<ModFilters>(NO_FILTERS);
  const [opened, setOpened] = useState<string | null>(null);
  const guard = useGuardStatus(installationId);
  const reconcile = useReconcile(installationId);
  // The list is only shown after the disk was compared with the database.
  const mods = useMods(installationId, !reconcile.isPending);

  const notice = guard.data ? guardNotice(guard.data) : null;
  const blockedReason = guard.data
    ? (notice?.text ?? null)
    : guard.isPending
      ? "Checking whether the game is running…"
      : null;

  const list = mods.data ?? [];
  const openedMod: ModDto | undefined = list.find((m) => m.id === opened);
  const groups = CATEGORIES.map((category) => {
    const all = list.filter((m) => m.category === category);
    return { category, all, visible: all.filter((m) => matchesFilters(m, filters)) };
  }).filter((g) => g.visible.length > 0);

  return (
    <Tooltip.Provider delayDuration={200}>
      <p className={styles.muted}>{name}</p>
      {notice ? <GuardBanner notice={notice} /> : null}
      {guard.error ? <ErrorMessage error={guard.error} /> : null}
      {reconcile.error ? <ErrorMessage error={reconcile.error} /> : null}
      {mods.error ? <ErrorMessage error={mods.error} /> : null}
      {reconcile.isPending ? <p className={styles.muted}>Checking files…</p> : null}
      {mods.data ? (
        <>
          <VanillaPanel installationId={installationId} blockedReason={blockedReason} />
          <ModsToolbar
            filters={filters}
            onChange={setFilters}
            actions={
              <ImportZipButton installationId={installationId} blockedReason={blockedReason} />
            }
          />
          {list.length === 0 ? (
            <p className={styles.muted}>
              No managed mods yet. Scan the installation on the{" "}
              <Link to="/review">Review</Link> page and adopt candidates as mods.
            </p>
          ) : groups.length === 0 ? (
            <p className={styles.muted}>No mods match the current filters.</p>
          ) : (
            groups.map((g) => (
              <ModGroup
                key={g.category}
                installationId={installationId}
                category={g.category}
                all={g.all}
                visible={g.visible}
                blockedReason={blockedReason}
                checking={reconcile.isFetching}
                onOpen={(m) => setOpened(m.id)}
                onCheckAgain={() => void reconcile.refetch()}
              />
            ))
          )}
        </>
      ) : null}
      {openedMod ? (
        <ModDetailsDialog
          installationId={installationId}
          mod={openedMod}
          blockedReason={blockedReason}
          onClose={() => setOpened(null)}
        />
      ) : null}
    </Tooltip.Provider>
  );
}
