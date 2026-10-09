use std::path::Path;

use tresse_lib::VersionLabel;
use tresse_lib::commit::{CommitOptions, CommitOutcome, NoJournal, commit};
use tresse_lib::operation::{OperationKind, Step, StepKind};
use tresse_lib::partial_sync::pull_remote_content;

use crate::cas::{
    ConfiguredRemote, DEFAULT_CHUNK_RAW_SIZE, DEFAULT_MAX_OBJECT_SIZE, EmptyRemote,
    load_merged_meta, open_local_store,
};
use crate::config::TresseConfig;
use crate::fs_writer::{DiskSource, build_shape};

pub fn run(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    run_with(root, || Ok(()))
}

pub fn run_with(
    root: &Path,
    write: impl FnOnce() -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tresse_dir = root.join(".tresse");
    if !tresse_dir.is_dir() {
        return Err("not a tresse repository; run `tresse init` first".into());
    }

    // До чтения диска: оборванная до нас операция могла оставить рабочую
    // директорию позади проекции, и коммит принял бы недоехавшие файлы за
    // удалённые.
    let mut operation =
        crate::operation::begin(root, OperationKind::Commit, [Step::new(StepKind::Commit)])?;
    operation.start(StepKind::Commit)?;
    write()?;
    let config = TresseConfig::load(root)?;
    let mut local = open_local_store(root)?;
    let meta = load_merged_meta(&local)?;

    // App writes can occur within one millisecond and keep the same byte size.
    // Re-read the worktree instead of declaring those edits unchanged from stat.
    let ignore = config.ignore_rules()?;
    let (skel, current_index) = build_shape(root, &ignore, None)?;
    let disk_source = DiskSource(root.to_path_buf());

    let missing_base_content =
        tresse_lib::partial_sync::content_fetch_plan_paths(&meta, &skel.changed_paths(), &local)?;
    if !missing_base_content.is_empty() {
        let remote_config = config
            .origin()
            .ok_or("local content is incomplete and no remote is configured")?;
        let repo_key = tresse_lib::crypto::decode_key_b64(&remote_config.encryption_key)?;
        let remote = ConfiguredRemote::open(remote_config)?;
        pull_remote_content(&mut local, &remote, &repo_key, &missing_base_content)?;
    }

    let outcome = commit(
        &meta,
        &skel,
        &disk_source,
        &mut local,
        &EmptyRemote,
        CommitOptions {
            chunk_raw_size: DEFAULT_CHUNK_RAW_SIZE,
            max_object_size: DEFAULT_MAX_OBJECT_SIZE,
        },
        &mut NoJournal,
    )?;
    match outcome {
        CommitOutcome::NoChange => println!("no changes detected"),
        CommitOutcome::Committed {
            version,
            root_object,
            ..
        } => {
            println!(
                "committed version {} ({})",
                VersionLabel::from_source(version.source).format(),
                &root_object[..12]
            );
        }
    }
    crate::file_index::save(root, &current_index)?;
    operation.finish(StepKind::Commit)?;
    operation.complete()?;
    Ok(())
}
