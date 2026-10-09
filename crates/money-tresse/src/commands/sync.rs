use std::collections::HashSet;
use std::path::Path;

use rdx_sync::{ChunkClass, ObjectStore};
use tresse_lib::operation::{OperationKind, Step, StepKind};
use tresse_lib::partial_sync::pull_remote_meta;

use crate::cas::{ConfiguredRemote, open_local_store};
use crate::config::TresseConfig;

pub fn run(root: &Path) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let config = TresseConfig::load(root)?;
    let remote = config.origin().ok_or("remote is not configured")?;
    let repo_key = tresse_lib::crypto::decode_key_b64(&remote.encryption_key)?;
    let mut remote = ConfiguredRemote::open(remote)?;

    exchange(root, &mut remote, &repo_key)
}

pub fn exchange(
    root: &Path,
    remote: &mut impl rdx_sync::RemoteKv,
    repo_key: &[u8; 32],
) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    // Sync только обменивается объектами: рабочую директорию он не читает и не
    // пишет. Поэтому чужой долг перед ней он не гасит, а забирает в свою запись,
    // и свой — пути скачанной меты — оставляет там же шагом `restore`. Перельёт
    // их операция, работающая с файлами: commit, restore или revert.
    let mut operation = crate::operation::begin_carrying(
        root,
        OperationKind::Sync,
        [
            Step::new(StepKind::PullMeta),
            Step::new(StepKind::Push),
            Step::new(StepKind::Restore),
        ],
    )?;
    let mut local = open_local_store(root)?;

    // Скачанная meta видна сразу, а рабочая директория её ещё не отражает. Какие
    // пути она задела, записывается вместе с закрытием шага — и при ошибке тоже,
    // по тому, что успело доехать. Иначе неудавшийся pull (нет сети) оставлял бы
    // долг «перелить всё», и коммит ждал бы, пока скачается всё содержимое.
    let known: HashSet<String> = local.list_objects(ChunkClass::Meta)?.into_iter().collect();
    operation.start(StepKind::PullMeta)?;
    let pull = pull_remote_meta(&mut local, remote, repo_key);
    let landed: Vec<String> = local
        .list_objects(ChunkClass::Meta)?
        .into_iter()
        .filter(|id| !known.contains(id))
        .collect();
    let fetched_meta = landed
        .iter()
        .map(|id| {
            let bytes = local.get_bytes(ChunkClass::Meta, id)?;
            Ok(tresse_lib::meta_bundle::decode_meta_object(id, &bytes)?)
        })
        .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?;
    let restore_paths = tresse_lib::meta_bundle::tree_paths(&fetched_meta);
    operation
        .finish_with_restore_paths(StepKind::PullMeta, restore_paths.iter().cloned().collect())?;
    let pulled = match pull {
        Ok(report) => report.pulled,
        Err(error) => {
            operation.complete_or_leave()?;
            return Err(error.into());
        }
    };

    operation.start(StepKind::Push)?;
    let mut pushed = 0;
    // Content раньше meta: удалённое хранилище не должно показывать версию, чьё
    // содержимое ещё не доехало, — чужая реплика не смогла бы её перелить.
    for class in [ChunkClass::Content, ChunkClass::Meta] {
        let ids = local.list_objects(class)?;
        pushed += rdx_sync::push_remote_objects(&local, remote, repo_key, class, &ids)?
            .pushed
            .len();
    }
    operation.finish(StepKind::Push)?;

    let pending = operation.leaves_restore();
    operation.complete_or_leave()?;

    println!("sync complete: fetched {}, pushed {}", pulled.len(), pushed);
    if pending {
        println!("the working tree is behind: run `tresse restore` to bring it up to date");
    }
    Ok((pulled.len(), pushed))
}
