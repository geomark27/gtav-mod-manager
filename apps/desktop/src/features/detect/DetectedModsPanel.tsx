import * as Checkbox from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import { useState } from "react";
import type { AdoptDetectedResultDto } from "../../bindings/AdoptDetectedResultDto";
import type { Category } from "../../bindings/Category";
import type { DetectedModDto } from "../../bindings/DetectedModDto";
import type { DetectedModsDto } from "../../bindings/DetectedModsDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { formatBytes } from "../../lib/format";
import { ErrorMessage } from "../installations/ErrorMessage";
import { CategorySelect } from "../inventory/CategorySelect";
import { CLASS_INFO } from "../inventory/labels";
import inventoryStyles from "../inventory/Inventory.module.css";
import styles from "./Detect.module.css";
import { CONFIDENCE_INFO, requirementInfo } from "./labels";
import { useAdoptDetected, useDetectedMods } from "./useDetect";

/** Maximum mod name length, in characters. */
const MAX_NAME_CHARS = 120;

interface Draft {
  selected: boolean;
  name: string;
  category: Category;
}

export interface DetectedModsPanelProps {
  installationId: string;
  /** Why changes are blocked right now, or `null`. */
  blockedReason: string | null;
}

/** Mods proposed from the last scan, adopted together (one transaction each). */
export function DetectedModsPanel({ installationId, blockedReason }: DetectedModsPanelProps) {
  const detected = useDetectedMods(installationId);
  const [result, setResult] = useState<AdoptDetectedResultDto | null>(null);

  if (detected.isPending) return <p className={styles.muted}>Looking for mods…</p>;
  if (detected.error) return <ErrorMessage error={detected.error} />;
  const data = detected.data;

  return (
    <div className={styles.stack}>
      {result ? <ResultSummary result={result} /> : null}
      {data.scannedAt === null ? (
        <p className={styles.muted}>Scan the installation to detect the mods in its folder.</p>
      ) : data.proposals.length === 0 ? (
        <p className={styles.muted}>No unmanaged mod files were found in the last scan.</p>
      ) : (
        <ProposalsForm
          // A new scan or adoption brings new proposals: start from their suggestions again.
          key={`${data.scannedAt}|${data.proposals.map((p) => p.key).join(",")}`}
          installationId={installationId}
          data={data}
          blockedReason={blockedReason}
          onAdopted={setResult}
        />
      )}
      {data.ambiguous.length > 0 ? (
        <div className={styles.card}>
          <strong>Files that match more than one known mod</strong>
          <p className={styles.muted}>
            These files are left out of the proposals. Adopt them by hand from the All files tab.
          </p>
          <ul className={styles.files}>
            {data.ambiguous.map((a) => (
              <li key={a.relPath}>
                {a.relPath} <span className={styles.muted}>({a.candidates.join(", ")})</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

function initialDrafts(proposals: DetectedModDto[]): Record<string, Draft> {
  return Object.fromEntries(
    proposals.map((p) => [
      p.key,
      // Known mods are preselected; grouped files must be checked first.
      { selected: p.sourceKind === "recognized" && !p.nameTaken, name: p.name, category: p.category },
    ]),
  );
}

interface FormProps {
  installationId: string;
  data: DetectedModsDto;
  blockedReason: string | null;
  onAdopted: (result: AdoptDetectedResultDto) => void;
}

function ProposalsForm({ installationId, data, blockedReason, onAdopted }: FormProps) {
  const [drafts, setDrafts] = useState(() => initialDrafts(data.proposals));
  const [confirmUnknown, setConfirmUnknown] = useState(false);
  const adopt = useAdoptDetected(installationId);

  const update = (key: string, change: Partial<Draft>) =>
    setDrafts((cur) => {
      const current = cur[key];
      return current ? { ...cur, [key]: { ...current, ...change } } : cur;
    });

  const chosen = data.proposals.filter((p) => drafts[p.key]?.selected);
  const names = chosen.map((p) => (drafts[p.key]?.name ?? "").trim());
  const lowered = names.map((n) => n.toLowerCase());
  const problem =
    chosen.length === 0
      ? "Select at least one mod."
      : names.some((n) => n.length === 0 || n.length > MAX_NAME_CHARS)
        ? "Every selected mod needs a name of 1 to 120 characters."
        : new Set(lowered).size !== lowered.length
          ? "Two selected mods have the same name."
          : null;
  const needsConfirm = chosen.some((p) => p.includesUnknown);
  const disabledReason =
    blockedReason ?? problem ?? (needsConfirm && !confirmUnknown ? "Confirm the unrecognized files first." : null);

  const submit = () =>
    adopt.mutate(
      {
        confirmUnknown,
        items: chosen.map((p) => {
          const d = drafts[p.key];
          return {
            signatureId: p.signatureId,
            name: (d?.name ?? p.name).trim(),
            category: d?.category ?? p.category,
            relPaths: p.files.map((f) => f.relPath),
          };
        }),
      },
      { onSuccess: onAdopted },
    );

  return (
    <div className={styles.stack}>
      <p className={styles.muted}>
        Adopting keeps every file where it is and stores a verified copy in the vault. Check the
        names and files, then adopt the selected mods.
      </p>
      <ul className={styles.list}>
        {data.proposals.map((p) => {
          const d = drafts[p.key];
          if (!d) return null;
          return (
            <ProposalCard
              key={p.key}
              proposal={p}
              draft={d}
              disabled={adopt.isPending}
              onChange={(change) => update(p.key, change)}
            />
          );
        })}
      </ul>
      {adopt.error ? <ErrorMessage error={adopt.error} /> : null}
      <div className={styles.footer}>
        {needsConfirm ? (
          <label className={styles.check}>
            <Checkbox.Root
              className={inventoryStyles.selectBox}
              checked={confirmUnknown}
              onCheckedChange={(v) => setConfirmUnknown(v === true)}
              disabled={adopt.isPending}
            >
              <Checkbox.Indicator>
                <Check size={12} aria-hidden="true" />
              </Checkbox.Indicator>
            </Checkbox.Root>
            Some selected files are not recognized; they are mine to adopt.
          </label>
        ) : (
          <span className={styles.muted}>
            {chosen.length} of {data.proposals.length} selected
          </span>
        )}
        <Button
          variant="primary"
          loading={adopt.isPending}
          disabled={disabledReason !== null}
          title={disabledReason ?? undefined}
          onClick={submit}
        >
          Adopt selected ({chosen.length})
        </Button>
      </div>
    </div>
  );
}

interface CardProps {
  proposal: DetectedModDto;
  draft: Draft;
  disabled: boolean;
  onChange: (change: Partial<Draft>) => void;
}

function ProposalCard({ proposal: p, draft, disabled, onChange }: CardProps) {
  const confidence = CONFIDENCE_INFO[p.confidence];
  return (
    <li className={styles.card} data-selected={draft.selected} aria-label={p.name}>
      <div className={styles.header}>
        <Checkbox.Root
          className={inventoryStyles.selectBox}
          aria-label={`Include ${p.name}`}
          checked={draft.selected}
          onCheckedChange={(v) => onChange({ selected: v === true })}
          disabled={disabled}
        >
          <Checkbox.Indicator>
            <Check size={12} aria-hidden="true" />
          </Checkbox.Indicator>
        </Checkbox.Root>
        <label className={`${inventoryStyles.field} ${styles.nameField}`}>
          <span>Name</span>
          <input
            className={inventoryStyles.input}
            value={draft.name}
            disabled={disabled}
            onChange={(e) => onChange({ name: e.target.value })}
          />
        </label>
        <CategorySelect
          label="Category"
          value={draft.category}
          disabled={disabled}
          onChange={(category) => onChange({ category })}
        />
      </div>
      <div className={styles.badges}>
        <span title={confidence.hint}>
          <Badge tone={confidence.tone}>{confidence.label}</Badge>
        </span>
        <span className={styles.muted}>
          {p.files.length} {p.files.length === 1 ? "file" : "files"} · {formatBytes(p.totalSize)}
        </span>
        {p.requires.map((r) => {
          const info = requirementInfo(r);
          return (
            <Badge key={r.signatureId} tone={info.tone}>
              Requires {info.text}
            </Badge>
          );
        })}
      </div>
      {!p.editionSupported ? (
        <p className={styles.warning}>This mod is not made for this edition of the game.</p>
      ) : null}
      {p.nameTaken ? (
        <p className={styles.warning}>A managed mod already has this name; choose another.</p>
      ) : null}
      <details className={styles.files}>
        <summary>Files</summary>
        <ul>
          {p.files.map((f) => (
            <li key={f.relPath}>
              {f.relPath}{" "}
              {f.classification === "unknown" ? (
                <Badge tone={CLASS_INFO.unknown.tone}>{CLASS_INFO.unknown.label}</Badge>
              ) : null}
            </li>
          ))}
        </ul>
      </details>
    </li>
  );
}

function ResultSummary({ result }: { result: AdoptDetectedResultDto }) {
  const n = result.adopted.length;
  return (
    <div className={styles.card} role="status">
      {n > 0 ? (
        <p className={styles.success}>
          Adopted {n} {n === 1 ? "mod" : "mods"}: {result.adopted.map((m) => m.name).join(", ")}
          {result.dependenciesLinked > 0
            ? ` · ${result.dependenciesLinked} ${result.dependenciesLinked === 1 ? "dependency" : "dependencies"} linked`
            : ""}
        </p>
      ) : null}
      {result.skipped.length > 0 ? (
        <>
          <p className={styles.warning}>Not adopted:</p>
          <ul className={styles.files}>
            {result.skipped.map((s) => (
              <li key={s.name}>
                {s.name}: {s.error.message}
                {s.error.hint ? ` ${s.error.hint}` : ""}
              </li>
            ))}
          </ul>
        </>
      ) : null}
    </div>
  );
}
