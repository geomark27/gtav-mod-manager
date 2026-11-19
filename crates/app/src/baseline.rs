//! The baseline capture use case.

use chrono::Utc;
use gmm_domain::{BaselineTrust, RelPath, Sha256};
use gmm_platform::StdFs;
use gmm_scanner::capture_baseline;
use serde::Serialize;
use ts_rs::TS;

use crate::app::App;
use crate::error::AppError;
use crate::installations::parse_id;
use crate::operation::{OperationKind, ProgressSink};
use crate::scan::rfc3339;

/// The baseline of an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BaselineDto {
    /// Number of files in the baseline.
    #[ts(type = "number")]
    pub entries: u64,
    /// Trust level.
    pub trust: BaselineTrust,
    /// Capture time, RFC 3339 UTC; `null` when not known (baseline read back from storage).
    pub captured_at: Option<String>,
}

impl App {
    /// Hashes every original-looking file and stores it as the baseline, replacing the
    /// previous one atomically. `trusted` records that the user verified the game files
    /// with the launcher just before.
    pub fn capture_baseline(
        &self,
        id: &str,
        trusted: bool,
        sink: &dyn ProgressSink,
    ) -> Result<BaselineDto, AppError> {
        let id = parse_id(id)?;
        self.run_operation(OperationKind::Baseline, sink, |op| {
            let inst = self.load_validated(id)?;
            let data = capture_baseline(&StdFs::new(), &inst.root_path, &mut |p| {
                op.report(p.files_seen, p.bytes_hashed, p.current.as_deref())
            })?;
            let entries: Vec<(RelPath, u64, Sha256)> = data
                .into_iter()
                .map(|e| (e.rel_path, e.size, e.sha256))
                .collect();
            let trust = if trusted {
                BaselineTrust::Trusted
            } else {
                BaselineTrust::Untrusted
            };
            let captured_at = Utc::now();
            self.lock_db()?
                .replace_baseline(id, &entries, trust, captured_at)?;
            Ok(BaselineDto {
                entries: entries.len() as u64,
                trust,
                captured_at: Some(rfc3339(captured_at)),
            })
        })
    }
}
