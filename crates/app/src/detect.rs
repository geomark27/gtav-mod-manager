//! Detected mods (Phase 4 R3): proposals from the last scan and their bulk adoption.
//!
//! Proposals come from the known-mod signatures (`recognized`) and the grouping heuristics
//! (`grouped`) applied to the adoptable files of the last scan. They are only proposals:
//! nothing is managed until the user adopts them. Bulk adoption runs **one transaction per
//! mod**, requirements first, records the provenance of each mod, links the dependencies the
//! signatures declare, and rescans once at the end.

use std::collections::{BTreeMap, HashMap, HashSet};

use gmm_domain::{
    Category, Classification, InstallationId, InventoryItem, ModId, ModSource, ModSourceKind,
    RelPath,
};
use gmm_scanner::{Confidence, Signatures, group_candidates, recognize};
use gmm_store::{Database, InventoryFilter, MAX_PAGE_SIZE};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::adopt::{AdoptFileDto, AdoptRequest, block_reason};
use crate::app::App;
use crate::error::{AppError, ErrorCode};
use crate::guards::GuardNeeds;
use crate::installations::{not_found, parse_id};
use crate::mods::ModDto;
use crate::operation::{OperationKind, ProgressSink};
use crate::scan::rfc3339;

/// How sure a proposal is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DetectionConfidence {
    /// A known-mod signature with an exact file name matched.
    High,
    /// A known-mod signature matched through wildcard patterns only.
    Medium,
    /// Related files grouped by folder or name; check before adopting.
    Low,
}

impl From<Confidence> for DetectionConfidence {
    fn from(c: Confidence) -> Self {
        match c {
            Confidence::High => Self::High,
            Confidence::Medium => Self::Medium,
            Confidence::Low => Self::Low,
        }
    }
}

/// Where a requirement of a proposal stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RequirementStatus {
    /// A managed mod provides it.
    Managed,
    /// Another proposal of this list provides it.
    Detected,
    /// Not found in the game folder.
    Missing,
}

/// A mod a proposal needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RequirementDto {
    /// Signature id of the required mod.
    pub signature_id: String,
    /// Its display name.
    pub name: String,
    /// Where it stands.
    pub status: RequirementStatus,
    /// Name of the managed mod that provides it (`managed` only).
    pub mod_name: Option<String>,
}

/// A proposed mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DetectedModDto {
    /// Stable key within one scan (`recognized:<signature>` or `grouped:<first file>`).
    pub key: String,
    /// `recognized` or `grouped`.
    pub source_kind: ModSourceKind,
    /// Signature id (`recognized` only).
    pub signature_id: Option<String>,
    /// Suggested name.
    pub name: String,
    /// Suggested category.
    pub category: Category,
    /// How sure the proposal is.
    pub confidence: DetectionConfidence,
    /// Its files, ordered.
    pub files: Vec<AdoptFileDto>,
    /// Total size of the files.
    #[ts(type = "number")]
    pub total_size: u64,
    /// True when a file is `unknown` (adoption needs the explicit confirmation).
    pub includes_unknown: bool,
    /// Mods it needs.
    pub requires: Vec<RequirementDto>,
    /// False when the signature does not list this installation's edition.
    pub edition_supported: bool,
    /// True when a managed mod already has the suggested name.
    pub name_taken: bool,
}

/// A file several signatures claim equally; it can only be adopted by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AmbiguousFileDto {
    /// Path relative to the installation root.
    pub rel_path: String,
    /// Names of the competing known mods.
    pub candidates: Vec<String>,
}

/// Proposals for an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DetectedModsDto {
    /// Start of the scan the proposals come from, RFC 3339 UTC; `null` when never scanned.
    pub scanned_at: Option<String>,
    /// Recognized mods first, then grouped ones; each by name.
    pub proposals: Vec<DetectedModDto>,
    /// Files claimed equally by several known mods.
    pub ambiguous: Vec<AmbiguousFileDto>,
}

/// One mod to adopt in a bulk adoption.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DetectedAdoptItem {
    /// Signature id when the proposal was recognized (recorded as provenance).
    pub signature_id: Option<String>,
    /// Name of the new mod.
    pub name: String,
    /// Category of the new mod.
    pub category: Category,
    /// Its files, relative to the installation root.
    pub rel_paths: Vec<String>,
}

/// Request of `adopt_detected`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdoptDetectedRequest {
    /// Installation the files belong to.
    pub installation_id: String,
    /// The mods to adopt.
    pub items: Vec<DetectedAdoptItem>,
    /// The user confirmed that unrecognized files are theirs to adopt.
    pub confirm_unknown: bool,
}

/// A proposal that was not adopted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkippedAdoptionDto {
    /// Name requested for the mod.
    pub name: String,
    /// Why it was skipped.
    pub error: AppError,
}

/// Outcome of a bulk adoption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AdoptDetectedResultDto {
    /// Mods created, in adoption order.
    pub adopted: Vec<ModDto>,
    /// Proposals that failed, with the reason; the others are unaffected.
    pub skipped: Vec<SkippedAdoptionDto>,
    /// Dependency links created from the signatures' requirements.
    #[ts(type = "number")]
    pub dependencies_linked: u64,
}

/// Every adoptable item (candidate/unknown, nothing blocking) of the last scan.
fn adoptable_items(db: &Database, id: InstallationId) -> Result<Vec<InventoryItem>, AppError> {
    let mut out = Vec::new();
    let mut offset = 0;
    loop {
        let page = db.list_inventory(
            id,
            &InventoryFilter {
                classifications: vec![Classification::Candidate, Classification::Unknown],
                search: None,
                limit: MAX_PAGE_SIZE,
                offset,
            },
        )?;
        let n = page.items.len() as u32;
        out.extend(page.items.into_iter().filter(|i| block_reason(i).is_none()));
        offset += n;
        if n == 0 || u64::from(offset) >= page.total {
            return Ok(out);
        }
    }
}

/// The managed mod that provides a signature: one recognized as it, else one whose files
/// meet the signature's required patterns.
fn managed_provider(
    db: &Database,
    signatures: &Signatures,
    id: InstallationId,
    signature_id: &str,
) -> Result<Option<(ModId, String)>, AppError> {
    let mods = db.list_mods(id)?;
    if let Some(m) = mods.iter().find(|m| {
        m.source.kind == ModSourceKind::Recognized
            && m.source.detail.as_deref() == Some(signature_id)
    }) {
        return Ok(Some((m.id, m.name.clone())));
    }
    let Some(sig) = signatures.get(signature_id) else {
        return Ok(None);
    };
    for m in &mods {
        let entries = db.list_mod_entries(m.id)?;
        let paths: Vec<&RelPath> = entries.iter().map(|e| &e.rel_path).collect();
        if sig.is_satisfied_by(&paths) {
            return Ok(Some((m.id, m.name.clone())));
        }
    }
    Ok(None)
}

fn file_dtos(files: &[RelPath], items: &HashMap<String, &InventoryItem>) -> Vec<AdoptFileDto> {
    files
        .iter()
        .filter_map(|f| items.get(&f.key()))
        .map(|i| AdoptFileDto {
            rel_path: i.rel_path.as_str().to_owned(),
            size: i.size,
            classification: i.classification,
        })
        .collect()
}

/// Orders items so that a recognized mod comes after the recognized mods it requires; the
/// rest keeps the request order.
fn requirements_first(
    items: Vec<DetectedAdoptItem>,
    signatures: &Signatures,
) -> Vec<DetectedAdoptItem> {
    let in_batch: HashSet<String> = items
        .iter()
        .filter_map(|i| i.signature_id.clone())
        .collect();
    let mut pending = items;
    let mut done: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let ready = pending.iter().position(|i| {
            i.signature_id
                .as_deref()
                .and_then(|s| signatures.get(s))
                .is_none_or(|sig| {
                    sig.requires
                        .iter()
                        .all(|r| !in_batch.contains(r) || done.contains(r))
                })
        });
        // A cycle cannot come from the validated catalog; fall back to the request order.
        let next = pending.remove(ready.unwrap_or(0));
        if let Some(s) = &next.signature_id {
            done.insert(s.clone());
        }
        out.push(next);
    }
    out
}

impl App {
    /// Proposes mods from the adoptable files of the last scan.
    pub fn detect_mods(&self, installation_id: &str) -> Result<DetectedModsDto, AppError> {
        let id = parse_id(installation_id)?;
        let db = self.lock_db()?;
        let inst = db.get_installation(id)?.ok_or_else(not_found)?;
        let Some(scan) = db.last_scan(id)? else {
            return Ok(DetectedModsDto {
                scanned_at: None,
                proposals: Vec::new(),
                ambiguous: Vec::new(),
            });
        };
        let items = adoptable_items(&db, id)?;
        let by_key: HashMap<String, &InventoryItem> =
            items.iter().map(|i| (i.rel_path.key(), i)).collect();
        let paths: Vec<RelPath> = items.iter().map(|i| i.rel_path.clone()).collect();
        let catalog = self.catalogs.get(inst.edition);
        let recognition = recognize(&self.signatures, catalog, inst.edition, &paths);
        let groups = group_candidates(catalog, &recognition.unrecognized);
        let taken: HashSet<String> = db
            .list_mods(id)?
            .into_iter()
            .map(|m| m.name.to_lowercase())
            .collect();
        let detected: HashSet<&str> = recognition
            .mods
            .iter()
            .map(|m| m.signature_id.as_str())
            .collect();

        let mut proposals = Vec::new();
        for m in &recognition.mods {
            let files = file_dtos(&m.files, &by_key);
            let mut requires = Vec::new();
            for r in &m.requires {
                let name = self
                    .signatures
                    .get(r)
                    .map_or_else(|| r.clone(), |s| s.name.clone());
                let (status, mod_name) = match managed_provider(&db, &self.signatures, id, r)? {
                    Some((_, n)) => (RequirementStatus::Managed, Some(n)),
                    None if detected.contains(r.as_str()) => (RequirementStatus::Detected, None),
                    None => (RequirementStatus::Missing, None),
                };
                requires.push(RequirementDto {
                    signature_id: r.clone(),
                    name,
                    status,
                    mod_name,
                });
            }
            proposals.push(DetectedModDto {
                key: format!("recognized:{}", m.signature_id),
                source_kind: ModSourceKind::Recognized,
                signature_id: Some(m.signature_id.clone()),
                name: m.name.clone(),
                category: m.category,
                confidence: m.confidence.into(),
                total_size: files.iter().map(|f| f.size).sum(),
                includes_unknown: files
                    .iter()
                    .any(|f| f.classification == Classification::Unknown),
                files,
                requires,
                edition_supported: m.edition_supported,
                name_taken: taken.contains(&m.name.to_lowercase()),
            });
        }
        for g in &groups {
            let files = file_dtos(&g.files, &by_key);
            proposals.push(DetectedModDto {
                key: format!(
                    "grouped:{}",
                    g.files.first().map(RelPath::key).unwrap_or_default()
                ),
                source_kind: ModSourceKind::Grouped,
                signature_id: None,
                name: g.name.clone(),
                category: g.category,
                confidence: g.confidence.into(),
                total_size: files.iter().map(|f| f.size).sum(),
                includes_unknown: files
                    .iter()
                    .any(|f| f.classification == Classification::Unknown),
                files,
                requires: Vec::new(),
                edition_supported: true,
                name_taken: taken.contains(&g.name.to_lowercase()),
            });
        }
        let ambiguous = recognition
            .ambiguous
            .iter()
            .map(|a| AmbiguousFileDto {
                rel_path: a.path.as_str().to_owned(),
                candidates: a
                    .signature_ids
                    .iter()
                    .map(|s| {
                        self.signatures
                            .get(s)
                            .map_or_else(|| s.clone(), |x| x.name.clone())
                    })
                    .collect(),
            })
            .collect();
        Ok(DetectedModsDto {
            scanned_at: Some(rfc3339(scan.started_at)),
            proposals,
            ambiguous,
        })
    }

    /// Adopts several proposals: one transaction per mod, requirements first. A proposal
    /// that fails is reported and the others go on. Dependencies declared by the signatures
    /// are linked when both mods are managed. The inventory is rescanned once at the end.
    pub fn adopt_detected(
        &self,
        req: AdoptDetectedRequest,
        sink: &dyn ProgressSink,
    ) -> Result<AdoptDetectedResultDto, AppError> {
        let id = parse_id(&req.installation_id)?;
        self.run_operation(OperationKind::Adopt, sink, |op| {
            if req.items.is_empty() {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    "Select at least one mod to adopt.",
                ));
            }
            self.load_validated(id)?;
            self.check_guards_id(id, GuardNeeds::default())?;
            let mut seen: HashMap<String, &str> = HashMap::new();
            for item in &req.items {
                if let Some(s) = &item.signature_id
                    && self.signatures.get(s).is_none()
                {
                    return Err(AppError::new(
                        ErrorCode::InvalidInput,
                        format!("Unknown known-mod signature \"{s}\"."),
                    ));
                }
                for raw in &item.rel_paths {
                    let key = RelPath::parse(raw).map(|r| r.key()).unwrap_or(raw.clone());
                    if let Some(other) = seen.insert(key, &item.name) {
                        return Err(AppError::new(
                            ErrorCode::InvalidInput,
                            "The same file is in two of the selected mods.",
                        )
                        .with_hint("Remove it from one of them.")
                        .with_details(format!("{raw} ({other}, {})", item.name)));
                    }
                }
            }

            let items = requirements_first(req.items.clone(), &self.signatures);
            let mut adopted = Vec::new();
            let mut skipped = Vec::new();
            let mut by_signature: BTreeMap<String, ModId> = BTreeMap::new();
            for (n, item) in items.iter().enumerate() {
                let label = format!("Adopting {}", item.name);
                op.report(n as u64, 0, Some(label.as_str()));
                let source = item
                    .signature_id
                    .as_ref()
                    .map_or_else(ModSource::grouped, ModSource::recognized);
                let one = AdoptRequest {
                    installation_id: req.installation_id.clone(),
                    rel_paths: item.rel_paths.clone(),
                    name: item.name.clone(),
                    category: item.category,
                    notes: None,
                    confirm_unknown: req.confirm_unknown,
                };
                match self.adopt_inner(id, one, source, false, op) {
                    Ok(dto) => {
                        if let (Some(s), Ok(mid)) =
                            (&item.signature_id, crate::mods::parse_mod_id(&dto.id))
                        {
                            by_signature.insert(s.clone(), mid);
                        }
                        adopted.push(dto);
                    }
                    Err(error) => {
                        tracing::warn!(name = %item.name, %error, "proposal not adopted");
                        skipped.push(SkippedAdoptionDto {
                            name: item.name.clone(),
                            error,
                        });
                    }
                }
            }

            let mut dependencies_linked = 0;
            {
                let db = self.lock_db()?;
                for (signature_id, mod_id) in &by_signature {
                    let Some(sig) = self.signatures.get(signature_id) else {
                        continue;
                    };
                    for r in &sig.requires {
                        let provider = match by_signature.get(r) {
                            Some(m) => Some(*m),
                            None => managed_provider(&db, &self.signatures, id, r)?.map(|p| p.0),
                        };
                        let Some(provider) = provider else { continue };
                        match db.add_dependency(*mod_id, provider, chrono::Utc::now()) {
                            Ok(()) => dependencies_linked += 1,
                            Err(e) => {
                                tracing::warn!(%mod_id, %provider, error = %e, "dependency not linked");
                            }
                        }
                    }
                }
            }
            if !adopted.is_empty() {
                self.rescan_best_effort(id, op);
            }
            // Read back after the links so `dependsOnCount` is current (I-07).
            let db = self.lock_db()?;
            let adopted = adopted
                .iter()
                .map(|d| {
                    crate::mods::parse_mod_id(&d.id)
                        .and_then(|m| crate::mods::read_mod_dto(&db, m))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(AdoptDetectedResultDto {
                adopted,
                skipped,
                dependencies_linked,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gmm_scanner::Catalogs;

    fn item(sig: Option<&str>, name: &str) -> DetectedAdoptItem {
        DetectedAdoptItem {
            signature_id: sig.map(str::to_owned),
            name: name.to_owned(),
            category: Category::ScriptsGameplay,
            rel_paths: Vec::new(),
        }
    }

    #[test]
    fn requirements_are_adopted_first() {
        let catalogs = Catalogs::load().unwrap();
        let sigs = Signatures::load(&catalogs).unwrap();
        let ordered = requirements_first(
            vec![
                item(Some("rampage-trainer"), "Rampage"),
                item(None, "Loose"),
                item(Some("script-hook-v-dotnet"), "SHVDN"),
                item(Some("script-hook-v"), "SHV"),
                item(Some("asi-loader"), "ASI"),
            ],
            &sigs,
        );
        let names: Vec<&str> = ordered.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["Loose", "SHV", "ASI", "SHVDN", "Rampage"]);
    }

    #[test]
    fn requirements_outside_the_batch_do_not_block() {
        let catalogs = Catalogs::load().unwrap();
        let sigs = Signatures::load(&catalogs).unwrap();
        let ordered = requirements_first(vec![item(Some("rampage-trainer"), "Rampage")], &sigs);
        assert_eq!(ordered.len(), 1);
    }
}
