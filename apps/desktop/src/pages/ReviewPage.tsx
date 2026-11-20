import { useState } from "react";
import { Link } from "react-router";
import type { Classification } from "../bindings/Classification";
import { Card } from "../components/Card";
import { Button } from "../components/Button";
import { AdoptDialog } from "../features/adopt/AdoptDialog";
import { BaselinePanel } from "../features/baseline/BaselinePanel";
import { DetectedModsPanel } from "../features/detect/DetectedModsPanel";
import { GuardBanner } from "../features/guards/GuardBanner";
import { guardNotice } from "../features/guards/blocking";
import { useGuardStatus } from "../features/guards/useGuardStatus";
import { ClassChips } from "../features/inventory/ClassChips";
import { InventoryTable } from "../features/inventory/InventoryTable";
import { ScanControl } from "../features/inventory/ScanControl";
import {
  useDebouncedValue,
  useInventory,
  useInventorySummary,
} from "../features/inventory/useInventory";
import inventoryStyles from "../features/inventory/Inventory.module.css";
import { ErrorMessage } from "../features/installations/ErrorMessage";
import { useSelectedInstallation } from "../features/installations/useInstallations";
import styles from "./Page.module.css";

const PAGE_SIZE = 100;

type Tab = "detected" | "files";
const SEARCH_DEBOUNCE_MS = 250;

/** Review page: scan the active installation and inspect every classified file. */
export function ReviewPage() {
  const selected = useSelectedInstallation();

  if (selected.isPending) {
    return (
      <div className={styles.page}>
        <h1 className={styles.title}>Review</h1>
        <p className={styles.muted}>Loading…</p>
      </div>
    );
  }

  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Review</h1>
      {selected.error ? <ErrorMessage error={selected.error} /> : null}
      {selected.data ? (
        <ReviewContent installationId={selected.data.id} name={selected.data.displayName} />
      ) : (
        <p className={styles.muted}>
          No active installation. <Link to="/installations">Go to Installations</Link> to add or
          select one.
        </p>
      )}
    </div>
  );
}

function ReviewContent({ installationId, name }: { installationId: string; name: string }) {
  const [tab, setTab] = useState<Tab>("detected");
  const guard = useGuardStatus(installationId);
  const notice = guard.data ? guardNotice(guard.data) : null;
  const blockedReason = guard.data
    ? (notice?.text ?? null)
    : guard.error
      ? null
      : "Checking whether the game is running…";

  return (
    <>
      <p className={styles.muted}>{name}</p>
      <Card title="Scan">
        <ScanControl installationId={installationId} />
      </Card>
      <Card title="Baseline">
        <BaselinePanel installationId={installationId} />
      </Card>
      {notice ? <GuardBanner notice={notice} /> : null}
      <div className={styles.tabs} role="tablist" aria-label="Review">
        <button
          type="button"
          role="tab"
          className={styles.tab}
          aria-selected={tab === "detected"}
          onClick={() => setTab("detected")}
        >
          Detected mods
        </button>
        <button
          type="button"
          role="tab"
          className={styles.tab}
          aria-selected={tab === "files"}
          onClick={() => setTab("files")}
        >
          All files
        </button>
      </div>
      {tab === "detected" ? (
        <section className={styles.section} aria-label="Detected mods" role="tabpanel">
          <DetectedModsPanel installationId={installationId} blockedReason={blockedReason} />
        </section>
      ) : (
        <FilesTab installationId={installationId} />
      )}
    </>
  );
}

function FilesTab({ installationId }: { installationId: string }) {
  const [classifications, setClassifications] = useState<Classification[]>([]);
  const [search, setSearch] = useState("");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [adopting, setAdopting] = useState(false);
  const [adoptedName, setAdoptedName] = useState<string | null>(null);
  const debounced = useDebouncedValue(search.trim(), SEARCH_DEBOUNCE_MS);

  const summary = useInventorySummary(installationId);
  const page = useInventory(installationId, {
    classifications,
    search: debounced === "" ? null : debounced,
    limit: PAGE_SIZE,
    offset,
  });

  const toggle = (c: Classification) => {
    setOffset(0);
    setClassifications((cur) => (cur.includes(c) ? cur.filter((x) => x !== c) : [...cur, c]));
  };

  const toggleSelected = (relPath: string) => {
    setAdoptedName(null);
    setSelected((cur) => {
      const next = new Set(cur);
      if (!next.delete(relPath)) next.add(relPath);
      return next;
    });
  };

  const filtered = classifications.length > 0 || debounced !== "";
  const emptyText = filtered
    ? "No files match the current filters."
    : summary.data?.lastScan
      ? "The last scan found no files."
      : "No scan yet. Scan the installation to see its files.";

  return (
    <>
      <section className={styles.section} aria-label="Files" role="tabpanel">
        <ClassChips
          counts={summary.data?.counts ?? []}
          selected={classifications}
          onToggle={toggle}
        />
        <input
          type="search"
          className={inventoryStyles.search}
          aria-label="Search files"
          placeholder="Search by path"
          value={search}
          onChange={(e) => {
            setOffset(0);
            setSearch(e.target.value);
          }}
        />
        {adoptedName ? (
          <p className={inventoryStyles.success} role="status">
            Mod adopted: {adoptedName}
          </p>
        ) : null}
        {selected.size > 0 ? (
          <div className={inventoryStyles.selectionBar}>
            <span>{selected.size} selected</span>
            <Button variant="primary" onClick={() => setAdopting(true)}>
              Adopt as mod…
            </Button>
            <Button onClick={() => setSelected(new Set())}>Clear selection</Button>
          </div>
        ) : null}
        {page.error ? <ErrorMessage error={page.error} /> : null}
        {page.data ? (
          <InventoryTable
            items={page.data.items}
            total={page.data.total}
            offset={offset}
            pageSize={PAGE_SIZE}
            emptyText={emptyText}
            onOffsetChange={setOffset}
            selection={{ selected, onToggle: toggleSelected }}
          />
        ) : null}
      </section>
      {adopting ? (
        <AdoptDialog
          installationId={installationId}
          relPaths={[...selected]}
          onClose={() => setAdopting(false)}
          onAdopted={(mod) => {
            setAdopting(false);
            setSelected(new Set());
            setAdoptedName(mod.name);
          }}
        />
      ) : null}
    </>
  );
}
