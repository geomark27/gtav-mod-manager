import type { DiscoveredInstallationDto } from "../../bindings/DiscoveredInstallationDto";
import { Badge } from "../../components/Badge";
import { Button } from "../../components/Button";
import { Card } from "../../components/Card";
import { ErrorMessage } from "./ErrorMessage";
import styles from "./Installations.module.css";
import { EDITION_LABEL, PLATFORM_LABEL } from "./labels";
import { StatusBadge } from "./StatusBadge";
import { useDiscoverInstallations, useRegisterInstallation } from "./useInstallations";

function registerBlockReason(c: DiscoveredInstallationDto): string | null {
  if (c.alreadyRegistered) return "Already registered.";
  if (!c.registrable) return "Cannot be registered until the problems listed are fixed.";
  return null;
}

function Candidate({ candidate }: { candidate: DiscoveredInstallationDto }) {
  const register = useRegisterInstallation();
  const reason = registerBlockReason(candidate);
  const reasonId = `reason-${candidate.rootPath}`;
  return (
    <li className={styles.candidate}>
      <div className={styles.badges}>
        {candidate.edition ? (
          <Badge tone="info">{EDITION_LABEL[candidate.edition]}</Badge>
        ) : (
          <Badge tone="neutral">Unknown edition</Badge>
        )}
        <Badge tone="neutral">{PLATFORM_LABEL[candidate.platform]}</Badge>
        <StatusBadge status={candidate.status} />
      </div>
      <p className={styles.path}>{candidate.rootPath}</p>
      {candidate.problems.length > 0 ? (
        <ul className={styles.problems}>
          {candidate.problems.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      ) : null}
      {candidate.missingFiles.length > 0 ? (
        <p className={styles.muted}>
          Missing files:{" "}
          <span className={styles.mono}>{candidate.missingFiles.join(", ")}</span>
        </p>
      ) : null}
      {register.error ? <ErrorMessage error={register.error} /> : null}
      <div className={styles.actions}>
        <Button
          variant="primary"
          disabled={reason !== null}
          loading={register.isPending}
          aria-describedby={reason ? reasonId : undefined}
          onClick={() =>
            register.mutate({
              rootPath: candidate.rootPath,
              displayName: null,
              platform: candidate.platform,
            })
          }
        >
          Register
        </Button>
        {reason ? (
          <span id={reasonId} className={styles.muted}>
            {reason}
          </span>
        ) : null}
      </div>
    </li>
  );
}

/** Runs detection and lists the candidates found. */
export function DiscoverPanel() {
  const discover = useDiscoverInstallations();
  return (
    <Card title="Detect installations">
      <div className={styles.actions}>
        <Button
          variant="secondary"
          loading={discover.isPending}
          onClick={() => discover.mutate()}
        >
          Detect installations
        </Button>
      </div>
      {discover.error ? <ErrorMessage error={discover.error} /> : null}
      {discover.data ? (
        discover.data.length === 0 ? (
          <p className={styles.muted}>No GTA V or Red Dead Redemption 2 installations were detected.</p>
        ) : (
          <ul className={styles.candidates}>
            {discover.data.map((c) => (
              <Candidate key={c.rootPath} candidate={c} />
            ))}
          </ul>
        )
      ) : null}
    </Card>
  );
}
