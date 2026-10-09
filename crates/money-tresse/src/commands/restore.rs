use std::path::Path;

use tresse_lib::operation::{OperationKind, Step, StepKind};

use tresse_lib::VersionLabel;

use crate::cas::{load_merged_meta, open_local_store, resolve_version};
use crate::config::TresseConfig;

/// `tresse restore` — привести рабочую директорию в согласие с Tresse.
///
/// Без версии — перелить то, что рабочей директории задолжали прежние операции
/// (обычно пути, скачанные `tresse sync`), не трогая изменённое пользователем.
/// С версией — привести директорию к ней целиком; там перезапись и есть просьба.
pub fn run(root: &Path, version_spec: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let tresse_dir = root.join(".tresse");
    if !tresse_dir.is_dir() {
        return Err("not a tresse repository; run `tresse init` first".into());
    }
    let Some(version_spec) = version_spec else {
        return restore_pending(root);
    };

    let config = TresseConfig::load(root)?;
    // Версия проверяется до того, как попадёт в запись: запись с версией,
    // которой нет, не довести никогда, и она заперла бы репозиторий.
    let version = {
        let local = open_local_store(root)?;
        let target = resolve_version(&load_merged_meta(&local)?, version_spec)?;
        VersionLabel::from_source(target.source).format()
    };
    // Версия записана в шаге: оборванный restore доводится к ней же.
    let mut operation = crate::operation::begin(
        root,
        OperationKind::Restore,
        [Step::restore_version(version.as_str())],
    )?;
    let mut local = open_local_store(root)?;

    operation.start(StepKind::Restore)?;
    crate::worktree::restore_version(root, &config, &mut local, &version, false)?;
    operation.finish(StepKind::Restore)?;
    operation.complete()?;

    println!("restored version {version}");
    Ok(())
}

/// Перелить то, что осталось за прежней записью. Вся работа — в `begin`: он
/// делает это перед любой операцией, работающей с файлами.
fn restore_pending(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let operation =
        crate::operation::begin(root, OperationKind::Restore, [Step::new(StepKind::Restore)])?;
    match &operation.settled {
        Some(outcome) => println!("restored {}, removed {}", outcome.restored, outcome.removed),
        None => println!("the working tree is up to date"),
    }
    operation.complete()
}
