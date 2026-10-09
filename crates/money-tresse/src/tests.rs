use std::path::Path;
use std::process::Command;

use rdx_rs::{RdxElement, RdxValue, Stamp};
use rdx_sync::MemRemoteKv;

use crate::storage::dir::DirRemote;
use crate::{RemoteConfig, StorageType, TresseConfig, config};

fn remote() -> RemoteConfig {
    RemoteConfig {
        storage_type: StorageType::S3,
        repo_id: "ledger".into(),
        encryption_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
        s3_endpoint: "https://s3.example.com".into(),
        s3_bucket: "test".into(),
        s3_access_key_id: "test".into(),
        s3_secret_access_key: "test".into(),
        ..Default::default()
    }
}

fn setup(root: &Path) {
    crate::init(root).unwrap();
    config::write_remote(root, Some(&remote())).unwrap();
}

fn record(key: &str, source: u64, time: u64) -> RdxElement {
    RdxElement::new(RdxValue::Eulerian(vec![RdxElement::with_stamp(
        RdxValue::Tuple(vec![RdxElement::new(RdxValue::Str(key.into()))]),
        Stamp::new(source, time),
    )]))
}

fn write(root: &Path, doc: &RdxElement) {
    crate::edit(root, || {
        std::fs::write(root.join("ledger.rdx"), rdx_rs::encode(doc)).map_err(|e| e.to_string())
    })
    .unwrap();
}

fn read(root: &Path) -> RdxElement {
    rdx_rs::decode(&std::fs::read(root.join("ledger.rdx")).unwrap())
        .unwrap()
        .0
}

#[test]
fn remotes_are_read_from_disk_after_another_client_changes_them() {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    let mut changed = remote();
    changed.repo_id = "other".into();
    changed.write_origin(dir.path()).unwrap();
    assert_eq!(config::read_remote(dir.path()).unwrap(), Some(changed));
}

#[test]
fn edits_are_refused_while_another_client_holds_the_repository() {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    let _lock = crate::operation::RepoLock::acquire(dir.path()).unwrap();
    let mut called = false;
    let result = crate::edit(dir.path(), || {
        called = true;
        Ok(())
    });
    assert!(result.unwrap_err().contains("another tresse operation"));
    assert!(!called);
}

#[test]
fn equal_stat_metadata_does_not_hide_a_native_rdx_edit() {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    write(
        dir.path(),
        &RdxElement::with_stamp(RdxValue::Str("a".into()), Stamp::new(1, 64)),
    );
    let path = dir.path().join("ledger.rdx");
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    crate::edit(dir.path(), || {
        std::fs::write(
            &path,
            rdx_rs::encode(&RdxElement::with_stamp(
                RdxValue::Str("b".into()),
                Stamp::new(1, 128),
            )),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        Ok(())
    })
    .unwrap();
    let local = crate::cas::open_local_store(dir.path()).unwrap();
    assert_eq!(
        crate::cas::load_merged_meta(&local).unwrap().versions.len(),
        2
    );
}

#[test]
fn an_encrypted_directory_remote_exchanges_native_documents() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let remote_dir = tempfile::tempdir().unwrap();
    let mut transport = DirRemote::open(remote_dir.path());
    setup(a.path());
    setup(b.path());
    write(a.path(), &record("first", 1, 64));
    crate::sync_with(a.path(), &mut transport).unwrap();
    crate::sync_with(b.path(), &mut transport).unwrap();
    assert_eq!(read(a.path()), read(b.path()));
    write(
        b.path(),
        &rdx_rs::merge(&read(b.path()), &record("second", 2, 128)),
    );
    crate::sync_with(b.path(), &mut transport).unwrap();
    crate::sync_with(a.path(), &mut transport).unwrap();
    assert_eq!(read(a.path()), read(b.path()));
}

#[test]
fn an_idle_exchange_sends_no_objects() {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    write(dir.path(), &record("first", 1, 64));
    let mut transport = MemRemoteKv::new();
    crate::sync_with(dir.path(), &mut transport).unwrap();
    assert_eq!(
        crate::sync_with(dir.path(), &mut transport).unwrap(),
        (0, 0)
    );
}

#[test]
fn tracked_ignore_rules_are_respected_without_client_defaults() {
    let dir = tempfile::tempdir().unwrap();
    setup(dir.path());
    std::fs::write(
        dir.path().join("tresse.toml"),
        "[ignore]\npatterns = [\"private\"]\n[languages.rdx]\nstrategy = \"native\"\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("private"), "local").unwrap();
    std::fs::write(dir.path().join("visible"), "tracked").unwrap();
    crate::commit(dir.path()).unwrap();
    let local = crate::cas::open_local_store(dir.path()).unwrap();
    let meta = crate::cas::load_merged_meta(&local).unwrap();
    assert!(!meta.tree.contains_key("private"));
    assert!(meta.tree.contains_key("visible"));
}

#[test]
#[ignore = "requires TRESSE_CLI_BIN pointing to the matching upstream Tresse CLI"]
fn upstream_cli_and_embedded_client_share_config_history_locks_and_sync() {
    let cli = std::env::var_os("TRESSE_CLI_BIN").expect("set TRESSE_CLI_BIN");
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let remote_dir = tempfile::tempdir().unwrap();
    let run = |root: &Path, args: &[&str]| {
        let output = Command::new(&cli)
            .current_dir(root)
            .args(args)
            .env("TRESSE_TEST_REMOTE_DIR", remote_dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "CLI failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    setup(a.path());
    write(a.path(), &record("first", 1, 64));
    let exported = run(a.path(), &["config", "export"]);
    assert_eq!(
        RemoteConfig::from_share_string(exported.trim()).unwrap(),
        remote()
    );
    run(a.path(), &["config", "validate"]);
    let mut transport = DirRemote::open(remote_dir.path());
    crate::sync_with(a.path(), &mut transport).unwrap();
    run(b.path(), &["init"]);
    run(b.path(), &["config", "import", &exported]);
    assert_eq!(
        TresseConfig::load(b.path()).unwrap().origin(),
        Some(&remote())
    );
    run(b.path(), &["sync"]);
    // The embedded client resumes the CLI's pending restore journal.
    crate::sync_with(b.path(), &mut transport).unwrap();
    assert_eq!(read(a.path()), read(b.path()));
    let first_version =
        crate::cas::load_merged_meta(&crate::cas::open_local_store(b.path()).unwrap())
            .unwrap()
            .versions;
    let latest = first_version
        .iter()
        .max_by_key(|stamp| stamp.source)
        .unwrap();
    let label = tresse_lib::VersionLabel::from_source(latest.source).format();
    let amended = rdx_rs::merge(&read(b.path()), &record("second", 2, 128));
    std::fs::write(b.path().join("ledger.rdx"), rdx_rs::encode(&amended)).unwrap();
    run(b.path(), &["commit"]);
    run(b.path(), &["sync"]);
    crate::sync_with(a.path(), &mut transport).unwrap();
    assert_eq!(read(a.path()), amended);
    assert!(run(a.path(), &["log", "--changes"]).contains("ledger.rdx"));
    run(b.path(), &["restore", &label]);
    assert_eq!(read(b.path()), record("first", 1, 64));
    let _lock = crate::operation::RepoLock::acquire(a.path()).unwrap();
    let locked = Command::new(&cli)
        .current_dir(a.path())
        .arg("commit")
        .output()
        .unwrap();
    assert!(!locked.status.success());
    assert!(String::from_utf8_lossy(&locked.stderr).contains("another tresse operation"));
}
