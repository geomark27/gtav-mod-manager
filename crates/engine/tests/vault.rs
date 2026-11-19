//! Vault: storing, verification, never overwriting.
#![allow(clippy::expect_used, clippy::panic)]

mod common;

use common::Fixture;
use gmm_domain::{Sha256, TransactionId};
use gmm_engine::Error;
use gmm_platform::{Fs, StdFs, sha256_bytes};

fn sha_of(data: &[u8]) -> Sha256 {
    Sha256::parse(&sha256_bytes(data)).expect("sha")
}

#[test]
fn store_creates_verified_readonly_object_and_is_idempotent() {
    let fx = Fixture::new();
    let fs = StdFs;
    let src = fx.install.path("Trainer.asi");
    let data = fx.install.read("Trainer.asi").expect("read");
    let sha = sha_of(&data);
    let tx = TransactionId::new();

    assert!(!fx.vault.verify(&fs, &sha).expect("verify"));
    assert!(fx.vault.store_from(&fs, &src, &sha, tx).expect("store"));
    let object = fx.vault.object_path(&sha);
    assert!(object.ends_with(format!("objects/{}/{}", &sha.as_str()[..2], sha.as_str())));
    assert!(fx.vault.verify(&fs, &sha).expect("verify"));
    assert!(fs.metadata(&object).expect("meta").readonly);
    assert!(!fx.vault.store_from(&fs, &src, &sha, tx).expect("again"));
    assert!(fx.leftovers().is_empty());
}

#[test]
fn wrong_expected_hash_is_rejected_and_leaves_nothing() {
    let fx = Fixture::new();
    let fs = StdFs;
    let wrong = sha_of(b"something else");
    let err = fx
        .vault
        .store_from(
            &fs,
            &fx.install.path("Trainer.asi"),
            &wrong,
            TransactionId::new(),
        )
        .expect_err("must fail");
    assert!(matches!(err, Error::HashMismatch { .. }));
    assert!(!fs.exists(&fx.vault.object_path(&wrong)).expect("exists"));
    assert!(fx.leftovers().is_empty());
}

#[test]
fn corrupt_existing_object_is_never_overwritten() {
    let fx = Fixture::new();
    let fs = StdFs;
    let src = fx.install.path("Trainer.asi");
    let sha = sha_of(&fx.install.read("Trainer.asi").expect("read"));
    fx.vault
        .store_from(&fs, &src, &sha, TransactionId::new())
        .expect("store");
    let object = fx.vault.object_path(&sha);
    StdFs.set_readonly(&object, false).expect("make writable");
    std::fs::write(&object, b"corrupt").expect("corrupt");

    assert!(!fx.vault.verify(&fs, &sha).expect("verify"));
    let err = fx
        .vault
        .store_from(&fs, &src, &sha, TransactionId::new())
        .expect_err("must fail");
    assert!(matches!(err, Error::VaultCorrupt { .. }));
    assert_eq!(std::fs::read(&object).expect("read"), b"corrupt");
}
