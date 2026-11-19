//! Scan, inventory and baseline use cases against a real SQLite file and fake installs.
#![allow(clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use gmm_app::{
    App, AppDirs, AppError, AppOptions, ClassCountDto, ErrorCode, InventoryFilterDto, NullSink,
    ProgressDto, ProgressSink, RegisterInstallationRequest, ScanStatusDto,
};
use gmm_domain::{BaselineTrust, Classification, Edition, InstallationStatus};
use gmm_scanner::DiscoveryHints;
use gmm_scanner::testing::FakeOs;
use gmm_testkit::FakeInstall;
use tempfile::TempDir;

struct Env {
    _tmp: TempDir,
    app: Arc<App>,
    install: FakeInstall,
    id: String,
}

/// Legacy install with base files, one ASI plugin (candidate) and its INI and one unrecognized file.
fn env() -> Env {
    let tmp = TempDir::new().expect("tempdir");
    let base = dunce::canonicalize(tmp.path()).expect("canonical");
    let app = App::init_with(
        AppDirs::at(&base.join("data")),
        AppOptions {
            os: Arc::new(FakeOs::new()),
            hints: DiscoveryHints::default(),
            ..AppOptions::default()
        },
    )
    .expect("init");
    let install = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .asi_mod("trainer")
        .file("notes/readme.txt", b"hello".to_vec())
        .build()
        .expect("install");
    let dto = app
        .register_installation(RegisterInstallationRequest {
            root_path: install.root().to_string_lossy().into_owned(),
            display_name: None,
            platform: None,
        })
        .expect("register");
    Env {
        _tmp: tmp,
        app: Arc::new(app),
        install,
        id: dto.id,
    }
}

fn filter() -> InventoryFilterDto {
    InventoryFilterDto {
        classifications: vec![],
        search: None,
        limit: 100,
        offset: 0,
    }
}

fn count(counts: &[ClassCountDto], c: Classification) -> u64 {
    counts
        .iter()
        .find(|x| x.classification == c)
        .map_or(0, |x| x.count)
}

fn err<T>(r: Result<T, AppError>) -> AppError {
    match r {
        Ok(_) => panic!("expected an error"),
        Err(e) => e,
    }
}

/// Records every event.
#[derive(Default)]
struct Recorder(Mutex<Vec<ProgressDto>>);

impl ProgressSink for Recorder {
    fn emit(&self, p: &ProgressDto) {
        self.0.lock().expect("lock").push(p.clone());
    }
}

#[test]
fn scan_classifies_and_stores_the_inventory() {
    let e = env();
    let rec = Recorder::default();
    let s = e.app.scan_installation(&e.id, &rec).expect("scan");
    assert_eq!(s.files_seen, 9);
    assert_eq!(count(&s.counts, Classification::Original), 6);
    assert_eq!(count(&s.counts, Classification::Candidate), 1);
    assert_eq!(count(&s.counts, Classification::Unknown), 2);
    let names: Vec<_> = s.counts.iter().map(|c| c.classification.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);

    let events = rec.0.lock().expect("lock");
    let last = events.last().expect("events");
    assert!(last.done);
    assert_eq!(last.files_seen, 9);
    assert_eq!(events.iter().filter(|p| p.done).count(), 1);
    assert!(events.iter().all(|p| p.operation_id == last.operation_id));

    let summary = e.app.inventory_summary(&e.id).expect("summary");
    assert_eq!(summary.counts, s.counts);
    let scan = summary.last_scan.expect("last scan");
    assert_eq!(scan.status, ScanStatusDto::Completed);
    assert_eq!(scan.files_seen, 9);
    assert_eq!(scan.id, s.scan_id);
    assert_eq!(summary.baseline.trust, BaselineTrust::None);
}

#[test]
fn inventory_paging_filter_and_search() {
    let e = env();
    e.app.scan_installation(&e.id, &NullSink).expect("scan");

    let all = e.app.list_inventory(&e.id, filter()).expect("all");
    assert_eq!(all.total, 9);
    let paths: Vec<_> = all.items.iter().map(|i| i.rel_path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_by_key(|p| p.to_lowercase());
    assert_eq!(paths, sorted);

    let page = e
        .app
        .list_inventory(
            &e.id,
            InventoryFilterDto {
                limit: 3,
                offset: 6,
                ..filter()
            },
        )
        .expect("page");
    assert_eq!(page.total, 9);
    assert_eq!(page.items.len(), 3);

    let cand = e
        .app
        .list_inventory(
            &e.id,
            InventoryFilterDto {
                classifications: vec![Classification::Candidate],
                ..filter()
            },
        )
        .expect("candidates");
    assert_eq!(cand.total, 1);
    let item = &cand.items[0];
    assert!(item.rel_path.ends_with(".asi"));
    assert!(item.suggested_category.is_some());
    assert!(!item.evidence.is_empty());
    assert!(item.mtime_ns.parse::<i64>().is_ok());

    let found = e
        .app
        .list_inventory(
            &e.id,
            InventoryFilterDto {
                search: Some("  README ".into()),
                ..filter()
            },
        )
        .expect("search");
    assert_eq!(found.total, 1);
    assert_eq!(found.items[0].rel_path, "notes/readme.txt");

    let e2 = e.app.list_inventory("not-an-id", filter());
    assert_eq!(err(e2).code, ErrorCode::InvalidInput);
    let missing = e
        .app
        .list_inventory("0192f0e8-0000-7000-8000-000000000001", filter());
    assert_eq!(err(missing).code, ErrorCode::NotFound);
}

#[test]
fn second_scan_reuses_the_hash_cache() {
    let e = env();
    e.app
        .capture_baseline(&e.id, true, &NullSink)
        .expect("baseline");
    let first = e.app.scan_installation(&e.id, &NullSink).expect("scan 1");
    assert!(first.bytes_hashed > 0);
    let second = e.app.scan_installation(&e.id, &NullSink).expect("scan 2");
    assert_eq!(second.bytes_hashed, 0);
    assert_eq!(second.counts, first.counts);
}

#[test]
fn trusted_baseline_detects_original_modified() {
    let e = env();
    let b = e
        .app
        .capture_baseline(&e.id, true, &NullSink)
        .expect("baseline");
    assert_eq!(b.trust, BaselineTrust::Trusted);
    assert_eq!(b.entries, 9);
    assert!(b.captured_at.is_some());
    let summary = e.app.inventory_summary(&e.id).expect("summary");
    assert_eq!(summary.baseline.trust, BaselineTrust::Trusted);
    assert_eq!(summary.baseline.entries, 9);

    let before = e.app.scan_installation(&e.id, &NullSink).expect("scan");
    assert_eq!(count(&before.counts, Classification::OriginalModified), 0);

    e.install
        .write("update/update.rpf", b"tampered")
        .expect("write");
    let after = e.app.scan_installation(&e.id, &NullSink).expect("rescan");
    assert_eq!(count(&after.counts, Classification::OriginalModified), 1);
    let page = e
        .app
        .list_inventory(
            &e.id,
            InventoryFilterDto {
                classifications: vec![Classification::OriginalModified],
                ..filter()
            },
        )
        .expect("list");
    assert_eq!(page.items[0].rel_path, "update/update.rpf");
}

#[test]
fn untrusted_baseline_never_yields_original_modified() {
    let e = env();
    let b = e
        .app
        .capture_baseline(&e.id, false, &NullSink)
        .expect("baseline");
    assert_eq!(b.trust, BaselineTrust::Untrusted);
    e.install
        .write("update/update.rpf", b"tampered")
        .expect("write");
    let s = e.app.scan_installation(&e.id, &NullSink).expect("scan");
    assert_eq!(count(&s.counts, Classification::OriginalModified), 0);
    assert_eq!(count(&s.counts, Classification::Original), 6);
}

#[test]
fn a_new_baseline_replaces_the_previous_one() {
    let e = env();
    e.app.capture_baseline(&e.id, true, &NullSink).expect("1");
    e.install.write("extra.txt", b"x").expect("write");
    let b = e.app.capture_baseline(&e.id, false, &NullSink).expect("2");
    assert_eq!(b.entries, 10);
    assert_eq!(b.trust, BaselineTrust::Untrusted);
    let s = e.app.inventory_summary(&e.id).expect("summary");
    assert_eq!(s.baseline.entries, 10);
    assert_eq!(s.baseline.trust, BaselineTrust::Untrusted);
}

/// Blocks inside the first event until released.
struct BlockingSink {
    started: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
    first: AtomicBool,
}

impl ProgressSink for BlockingSink {
    fn emit(&self, _p: &ProgressDto) {
        if !self.first.swap(true, Ordering::SeqCst) {
            self.started.lock().expect("lock").send(()).expect("send");
            self.release.lock().expect("lock").recv().expect("recv");
        }
    }
}

#[test]
fn a_busy_slot_rejects_new_operations() {
    let e = env();
    let (started_tx, started_rx) = channel();
    let (release_tx, release_rx) = channel();
    let sink = BlockingSink {
        started: Mutex::new(started_tx),
        release: Mutex::new(release_rx),
        first: AtomicBool::new(false),
    };
    let app = Arc::clone(&e.app);
    let id = e.id.clone();
    let worker = std::thread::spawn(move || app.scan_installation(&id, &sink));
    started_rx.recv().expect("operation started");

    let busy = err(e.app.scan_installation(&e.id, &NullSink));
    assert_eq!(busy.code, ErrorCode::OperationInProgress);
    assert_eq!(
        busy.message,
        "Another operation is running. Wait for it to finish or cancel it."
    );
    let busy = err(e.app.capture_baseline(&e.id, true, &NullSink));
    assert_eq!(busy.code, ErrorCode::OperationInProgress);

    release_tx.send(()).expect("release");
    worker.join().expect("join").expect("first scan completes");
    e.app
        .scan_installation(&e.id, &NullSink)
        .expect("the slot is free again");
}

/// Cancels the running operation on its first event.
struct CancelSink {
    app: Arc<App>,
    events: AtomicU64,
}

impl ProgressSink for CancelSink {
    fn emit(&self, p: &ProgressDto) {
        self.events.fetch_add(1, Ordering::SeqCst);
        if !p.done {
            self.app.cancel_operation();
        }
    }
}

#[test]
fn cancelling_a_scan_records_it_and_keeps_the_inventory() {
    let e = env();
    e.app.scan_installation(&e.id, &NullSink).expect("scan");
    // More than 256 files so the scanner reports (and sees the cancel) mid-walk.
    for i in 0..300 {
        e.install
            .write(&format!("bulk/f{i:03}.txt"), b"x")
            .expect("write");
    }
    let sink = CancelSink {
        app: Arc::clone(&e.app),
        events: AtomicU64::new(0),
    };
    let r = e.app.scan_installation(&e.id, &sink);
    assert_eq!(err(r).code, ErrorCode::Cancelled);
    assert!(
        sink.events.load(Ordering::SeqCst) >= 2,
        "done event emitted"
    );

    let summary = e.app.inventory_summary(&e.id).expect("summary");
    let last = summary.last_scan.expect("last scan");
    assert_eq!(last.status, ScanStatusDto::Cancelled);
    assert_eq!(
        e.app.list_inventory(&e.id, filter()).expect("list").total,
        9
    );
    e.app
        .scan_installation(&e.id, &NullSink)
        .expect("slot freed and a new scan works");
}

#[test]
fn cancel_without_a_running_operation_is_a_no_op() {
    let e = env();
    e.app.cancel_operation();
    e.app.scan_installation(&e.id, &NullSink).expect("scan");
}

#[test]
fn cancelling_a_baseline_keeps_the_previous_one() {
    let e = env();
    e.app.capture_baseline(&e.id, true, &NullSink).expect("1");
    for i in 0..300 {
        e.install
            .write(&format!("bulk/f{i:03}.txt"), b"x")
            .expect("write");
    }
    let sink = CancelSink {
        app: Arc::clone(&e.app),
        events: AtomicU64::new(0),
    };
    let r = e.app.capture_baseline(&e.id, false, &sink);
    assert_eq!(err(r).code, ErrorCode::Cancelled);
    let s = e.app.inventory_summary(&e.id).expect("summary");
    assert_eq!(s.baseline.trust, BaselineTrust::Trusted);
    assert_eq!(s.baseline.entries, 9);
}

#[test]
fn a_missing_root_is_not_found_and_marked_missing() {
    let e = env();
    std::fs::remove_dir_all(e.install.root()).expect("remove");
    let r = e.app.scan_installation(&e.id, &NullSink);
    let error = err(r);
    assert_eq!(error.code, ErrorCode::NotFound);
    assert_eq!(error.message, "The installation folder was not found.");
    let inst = e.app.list_installations().expect("list");
    assert_eq!(inst[0].status, InstallationStatus::Missing);
    let r = e.app.capture_baseline(&e.id, true, &NullSink);
    assert_eq!(err(r).code, ErrorCode::NotFound);
    let r = e
        .app
        .scan_installation("0192f0e8-0000-7000-8000-000000000001", &NullSink);
    assert_eq!(err(r).code, ErrorCode::NotFound);
}

#[test]
fn dto_json_shapes() {
    let e = env();
    let s = e.app.scan_installation(&e.id, &NullSink).expect("scan");
    let v = serde_json::to_value(&s).expect("json");
    let obj = v.as_object().expect("object");
    let mut keys: Vec<_> = obj.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["bytesHashed", "counts", "filesSeen", "finishedAt", "scanId"]
    );
    assert_eq!(v["counts"][0]["classification"], "candidate");
    assert!(v["counts"][0]["count"].is_u64());

    let page = e.app.list_inventory(&e.id, filter()).expect("list");
    let item = serde_json::to_value(&page.items[0]).expect("json");
    let mut keys: Vec<_> = item
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "classification",
            "evidence",
            "firstSeenAt",
            "lastSeenAt",
            "mtimeNs",
            "relPath",
            "sha256",
            "size",
            "suggestedCategory"
        ]
    );
    assert!(item["mtimeNs"].is_string());

    let cand = e
        .app
        .list_inventory(
            &e.id,
            InventoryFilterDto {
                classifications: vec![Classification::Candidate],
                ..filter()
            },
        )
        .expect("cand");
    let ev = serde_json::to_value(&cand.items[0].evidence).expect("json");
    assert_eq!(ev[0]["kind"], "mod_pattern");

    let filter_json: InventoryFilterDto = serde_json::from_value(serde_json::json!({
        "classifications": ["original_modified"], "search": null, "limit": 100, "offset": 0
    }))
    .expect("filter");
    assert_eq!(
        filter_json.classifications,
        [Classification::OriginalModified]
    );

    let summary = serde_json::to_value(e.app.inventory_summary(&e.id).expect("sum")).expect("json");
    assert_eq!(summary["lastScan"]["status"], "completed");
    assert_eq!(summary["baseline"]["trust"], "none");
    assert!(summary["baseline"]["capturedAt"].is_null());
}
