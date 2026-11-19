//! Dashboard numbers and the baseline timestamp of the inventory summary.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::NullSink;
use gmm_domain::{BaselineTrust, Classification, ModState};

#[test]
fn counts_mods_vault_bytes_history_and_guards() {
    let e = env();
    let empty = e.app.dashboard(&e.id).expect("empty");
    assert_eq!(empty.mods.total, 0);
    assert_eq!(empty.vault_bytes, 0);
    assert!(empty.recent_transactions.is_empty());
    assert!(empty.last_scan.is_some(), "env() scanned once");
    assert_eq!(empty.baseline.trust, BaselineTrust::None);
    assert!(empty.baseline.captured_at.is_none());

    let trainer = adopt(&e, &TRAINER, "Trainer");
    let alpha = adopt(&e, &ALPHA, "Alpha");
    let beta = adopt(&e, &BETA, "Beta");
    set(&e, &beta.id, false);
    set(&e, &beta.id, true);
    set(&e, &beta.id, false);
    e.install
        .write("scripts/alpha.dll", b"edited")
        .expect("write");
    e.app.reconcile_installation(&e.id).expect("reconcile");
    assert_eq!(state_of(&e, &alpha.id), ModState::Drifted);

    let d = e.app.dashboard(&e.id).expect("dashboard");
    assert_eq!(
        (
            d.mods.total,
            d.mods.enabled,
            d.mods.disabled,
            d.mods.drifted,
            d.mods.error
        ),
        (3, 1, 1, 1, 0)
    );
    assert_eq!(state_of(&e, &trainer.id), ModState::Enabled);
    let expected: u64 = e
        .app
        .list_mods(&e.id)
        .expect("mods")
        .iter()
        .map(|m| m.total_size)
        .sum();
    assert!(expected > 0);
    assert_eq!(d.vault_bytes, expected);
    assert_eq!(
        d.recent_transactions.len(),
        5,
        "3 adopts + 3 toggles, newest 5"
    );
    assert_eq!(d.recent_transactions[0].description, "Disabled 1 mod");
    assert!(d.guard.game_running.is_empty());
    assert!(d.guard.writable);
    assert!(!d.guard.recovery_required);
    assert!(!d.guard.installation_missing);
    let managed = d
        .inventory
        .iter()
        .find(|c| c.classification == Classification::Managed)
        .map_or(0, |c| c.count);
    assert!(managed >= 2, "trainer and alpha are managed or drifted");

    *e.os.0.lock().expect("lock") = vec!["GTA5.exe".into()];
    assert_eq!(
        e.app
            .dashboard(&e.id)
            .expect("dashboard")
            .guard
            .game_running,
        ["GTA5.exe"]
    );

    let json = serde_json::to_value(&d).expect("json");
    for key in [
        "mods",
        "inventory",
        "lastScan",
        "baseline",
        "vaultBytes",
        "recentTransactions",
        "guard",
    ] {
        assert!(json.get(key).is_some(), "{key}");
    }
}

#[test]
fn baseline_reports_its_capture_time() {
    let e = env();
    let captured = e
        .app
        .capture_baseline(&e.id, true, &NullSink)
        .expect("baseline");
    let summary = e.app.inventory_summary(&e.id).expect("summary");
    assert_eq!(summary.baseline.trust, BaselineTrust::Trusted);
    assert_eq!(summary.baseline.entries, captured.entries);
    let at = summary.baseline.captured_at.as_ref().expect("captured at");
    assert!(at.ends_with('Z'));
    let d = e.app.dashboard(&e.id).expect("dashboard");
    assert_eq!(d.baseline, summary.baseline);
}
