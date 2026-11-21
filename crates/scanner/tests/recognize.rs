//! End to end: a fake installation is scanned, and its candidate/unknown files are recognized
//! and grouped. Original files (catalog or trusted signature) never reach a proposal.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use gmm_domain::{Classification, Edition, Game, RelPath};
use gmm_platform::StdFs;
use gmm_scanner::testing::FakeOs;
use gmm_scanner::{
    Catalogs, Confidence, ScanContext, ScanOutcome, Signatures, group_candidates, recognize, scan,
};
use gmm_testkit::FakeInstall;

fn scan_install(i: &FakeInstall, os: &FakeOs) -> ScanOutcome {
    let catalogs = Catalogs::load().unwrap();
    scan(
        &StdFs,
        os,
        catalogs.get(i.edition()),
        i.root(),
        &ScanContext::default(),
        &mut |_| true,
    )
    .unwrap()
}

fn adoptable(o: &ScanOutcome) -> Vec<RelPath> {
    o.items
        .iter()
        .filter(|i| i.classification.can_be_adopted())
        .map(|i| i.rel_path.clone())
        .collect()
}

#[test]
fn a_hand_modded_installation_becomes_named_mods_and_groups() {
    for edition in Game::Gtav.editions() {
        let i = FakeInstall::builder(edition)
            .with_base_files()
            .script_hook()
            .file("ScriptHookVDotNet.asi", b"shvdn".to_vec())
            .file("ScriptHookVDotNet3.dll", b"shvdn3".to_vec())
            .file("scripts/RampageTrainer.dll", b"rampage".to_vec())
            .file("scripts/Rampage/config.ini", b"cfg".to_vec())
            .file("scripts/CoolCars/cars.dll", b"cars".to_vec())
            .asi_mod("FastTravel")
            .build()
            .unwrap();
        let o = scan_install(&i, &FakeOs::new());
        let catalogs = Catalogs::load().unwrap();
        let catalog = catalogs.get(edition);
        let sigs = Signatures::load(&catalogs).unwrap();
        let r = recognize(&sigs, catalog, edition, &adoptable(&o));

        let ids: Vec<&str> = r.mods.iter().map(|m| m.signature_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "asi-loader",
                "rampage-trainer",
                "script-hook-v",
                "script-hook-v-dotnet"
            ],
            "{edition}"
        );
        let groups = group_candidates(catalog, &r.unrecognized);
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["CoolCars", "FastTravel"], "{edition}");
        assert_eq!(groups[1].files.len(), 2, "FastTravel.asi + FastTravel.ini");
        assert!(groups.iter().all(|g| g.confidence == Confidence::Low));

        // No proposal contains an original file.
        let originals: Vec<&RelPath> = o
            .items
            .iter()
            .filter(|x| x.classification == Classification::Original)
            .map(|x| &x.rel_path)
            .collect();
        for f in r
            .mods
            .iter()
            .flat_map(|m| &m.files)
            .chain(groups.iter().flat_map(|g| &g.files))
        {
            assert!(!originals.contains(&f), "{f} is original");
        }
    }
}

#[test]
fn a_file_signed_by_a_trusted_publisher_is_never_proposed() {
    let i = FakeInstall::builder(Edition::Legacy)
        .with_base_files()
        .script_hook()
        .build()
        .unwrap();
    let os = FakeOs::new().publisher(i.path("ScriptHookV.dll"), "Rockstar Games, Inc.");
    let o = scan_install(&i, &os);
    let catalogs = Catalogs::load().unwrap();
    let sigs = Signatures::load(&catalogs).unwrap();
    let r = recognize(
        &sigs,
        catalogs.get(Edition::Legacy),
        Edition::Legacy,
        &adoptable(&o),
    );
    let ids: Vec<&str> = r.mods.iter().map(|m| m.signature_id.as_str()).collect();
    assert_eq!(ids, ["asi-loader"]);
}
