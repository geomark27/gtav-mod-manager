//! Transaction history: ordering, descriptions, names and paging.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use gmm_app::{ErrorCode, NullSink};
use gmm_domain::{TransactionKind, TransactionStatus};

#[test]
fn lists_newest_first_with_readable_descriptions() {
    let e = env();
    let page = e.app.list_transactions(&e.id, 50, 0).expect("empty");
    assert_eq!((page.items.len(), page.total), (0, 0));

    let a = adopt(&e, &TRAINER, "Trainer");
    let b = adopt(&e, &ALPHA, "Alpha");
    set(&e, &a.id, false);
    set(&e, &b.id, false);
    e.app
        .set_category_enabled(
            &e.id,
            gmm_domain::Category::ScriptsGameplay,
            true,
            &NullSink,
        )
        .expect("category");

    let page = e.app.list_transactions(&e.id, 50, 0).expect("page");
    assert_eq!(page.total, 5);
    let descriptions: Vec<&str> = page.items.iter().map(|t| t.description.as_str()).collect();
    assert_eq!(
        descriptions,
        [
            "Switched a category (2 mods changed)",
            "Disabled 1 mod",
            "Disabled 1 mod",
            "Adopted mod “Alpha”",
            "Adopted mod “Trainer”",
        ]
    );
    assert_eq!(page.items[3].kind, TransactionKind::Adopt);
    assert_eq!(page.items[3].mod_names, ["Alpha"]);
    assert_eq!(page.items[1].mod_names, ["Alpha"]);
    assert_eq!(page.items[0].mod_names, ["Alpha", "Trainer"]);
    for t in &page.items {
        assert_eq!(t.status, TransactionStatus::Committed);
        assert!(t.finished_at.is_some());
        assert!(t.error.is_none());
        assert!(t.step_count > 0);
        assert!(t.created_at.ends_with('Z'));
    }
    assert_eq!(page.items[0].step_count, 4);

    // Paging.
    let second = e.app.list_transactions(&e.id, 2, 2).expect("second");
    assert_eq!(second.total, 5);
    let ids: Vec<&str> = second.items.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, [page.items[2].id.as_str(), page.items[3].id.as_str()]);
    assert_eq!(
        e.app
            .list_transactions(&e.id, 0, 0)
            .expect("clamped")
            .items
            .len(),
        1
    );

    // A forgotten mod's name is no longer resolved (the adopt name stays in its own entry).
    e.app.forget_mod(&b.id).expect("forget");
    let page = e.app.list_transactions(&e.id, 50, 0).expect("page");
    assert_eq!(page.items[0].mod_names, ["Trainer"]);
    assert_eq!(page.items[1].mod_names, Vec::<String>::new());
    assert_eq!(page.items[1].description, "Disabled 1 mod");
    assert_eq!(page.items[3].description, "Adopted mod “Alpha”");

    let json = serde_json::to_value(&page.items[0]).expect("json");
    assert_eq!(json["kind"], "group");
    assert!(json["stepCount"].is_number() && json["modNames"].is_array());
}

#[test]
fn unknown_installation_is_not_found() {
    let e = env();
    let other = gmm_domain::InstallationId::new().to_string();
    assert_eq!(
        err(e.app.list_transactions(&other, 10, 0)).code,
        ErrorCode::NotFound
    );
}
