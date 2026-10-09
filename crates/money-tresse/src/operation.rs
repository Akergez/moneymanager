//! Блокировка репозитория и запись операции на диске.
//!
//! Формат записи и решение, что доводить после обрыва, принадлежат домену —
//! [`tresse_lib::operation`]. Здесь то, что знает про диск и процессы: блокировка
//! ОС на `.tresse/lock`, отметка «жив» для фронтендов без неё и атомарная запись
//! файла операции.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tresse_lib::layout::{RESTORE_TEMP_DIR, TRESSE_DIR};
use tresse_lib::operation::{
    HEARTBEAT_INTERVAL_MS, LOCK_PATH, LockRecord, OS_LOCK_PATH, Operation, OperationKind, PATH,
    Resume, Step, StepKind, resume_unreadable,
};

use crate::config::TresseConfig;
use crate::worktree::RestoreOutcome;

type Failure = Box<dyn std::error::Error>;

const APP: &str = "money-manager";

/// Блокировка репозитория на время одной операции.
///
/// Блокировку ОС отпускает сама система, когда процесс умирает, — зависших
/// блокировок от CLI не бывает. Отметка времени в соседнем файле нужна тем, кто
/// блокировку ОС взять не может: по ней плагин узнаёт, что CLI ещё работает.
pub struct RepoLock {
    record: File,
    _os_lock: File,
    stop: Option<Sender<()>>,
    heartbeat: Option<JoinHandle<()>>,
}

impl RepoLock {
    pub fn acquire(root: &Path) -> Result<Self, Failure> {
        let open = |path: &str| {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join(path))
        };
        let os_lock = open(OS_LOCK_PATH)?;
        let mut record = open(LOCK_PATH)?;
        match os_lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(busy(read_record(&mut record)).into()),
            Err(TryLockError::Error(error)) => return Err(error.into()),
        }
        // Блокировка ОС наша, но плагин её не берёт: его слышно только по отметке.
        if let Some(owner) = read_record(&mut record)
            && !owner.os_lock
            && owner.is_fresh(now_ms())
        {
            return Err(busy(Some(owner)).into());
        }

        write_record(&mut record)?;
        let mut beating = record.try_clone()?;
        let (stop, stopped) = mpsc::channel::<()>();
        let heartbeat = std::thread::spawn(move || {
            let interval = Duration::from_millis(HEARTBEAT_INTERVAL_MS);
            while stopped.recv_timeout(interval) == Err(RecvTimeoutError::Timeout) {
                let _ = write_record(&mut beating);
            }
        });
        Ok(Self {
            record,
            _os_lock: os_lock,
            stop: Some(stop),
            heartbeat: Some(heartbeat),
        })
    }
}

impl Drop for RepoLock {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
        // Пустой файл — «свободно»: плагину не придётся ждать, пока истечёт аренда.
        let _ = self.record.set_len(0);
    }
}

fn read_record(file: &mut File) -> Option<LockRecord> {
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_to_end(&mut bytes).ok()?;
    LockRecord::from_json(&bytes)
}

fn write_record(file: &mut File) -> std::io::Result<()> {
    let bytes = LockRecord {
        app: APP.to_string(),
        pid: Some(std::process::id()),
        os_lock: true,
        heartbeat_ms: now_ms(),
        nonce: None,
    }
    .to_json();
    // Сначала новое содержимое, потом длина: файл ни на миг не остаётся пустым,
    // а пустой читается как «свободно».
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&bytes)?;
    file.set_len(bytes.len() as u64)?;
    file.flush()
}

fn busy(owner: Option<LockRecord>) -> String {
    match owner {
        Some(LockRecord {
            app,
            pid: Some(pid),
            ..
        }) => format!("another tresse operation is running ({app}, pid {pid})"),
        Some(LockRecord { app, .. }) => format!("another tresse operation is running ({app})"),
        None => "another tresse operation is running".to_string(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Идущая операция: блокировка плюс её запись на диске.
///
/// Запись остаётся лежать, если операция не дошла до [`RunningOperation::complete`], —
/// упала, была убита или вернула ошибку — либо дошла, но рабочую директорию
/// оставила позади проекции. Разницы нет: следующая операция, работающая с
/// файлами, сначала перельёт то, что за записью осталось.
pub struct RunningOperation {
    root: PathBuf,
    operation: Operation,
    /// Что перелито в рабочую директорию за прежней записью, если было что.
    pub settled: Option<RestoreOutcome>,
    _lock: RepoLock,
}

/// Взять блокировку, перелить оставшееся за прежней записью и начать свою.
///
/// Для операций, которые читают или пишут рабочую директорию: commit, restore,
/// revert. Коммит, начатый поверх отставшей директории, принял бы недоехавшие
/// файлы за удалённые.
pub fn begin(
    root: &Path,
    kind: OperationKind,
    steps: impl IntoIterator<Item = Step>,
) -> Result<RunningOperation, Failure> {
    let lock = RepoLock::acquire(root)?;
    let settled = settle(root)?;
    start(root, lock, Operation::new(kind, steps), settled)
}

/// Взять блокировку и начать свою операцию, забрав долг прежней записи себе.
///
/// Для операций, которые рабочую директорию не трогают: sync и gc. Они делают
/// ровно то, что названо, и файлов не переливают — но и долг не теряют.
pub fn begin_carrying(
    root: &Path,
    kind: OperationKind,
    steps: impl IntoIterator<Item = Step>,
) -> Result<RunningOperation, Failure> {
    let lock = RepoLock::acquire(root)?;
    let mut operation = Operation::new(kind, steps);
    if let Some((_, left)) = read_left(root)? {
        operation.carry(left);
    }
    start(root, lock, operation, None)
}

fn start(
    root: &Path,
    lock: RepoLock,
    operation: Operation,
    settled: Option<RestoreOutcome>,
) -> Result<RunningOperation, Failure> {
    // Обломки оборванного restore: под блокировкой они ничьи.
    let _ = fs::remove_dir_all(root.join(TRESSE_DIR).join(RESTORE_TEMP_DIR));
    let running = RunningOperation {
        root: root.to_path_buf(),
        operation,
        settled,
        _lock: lock,
    };
    running.save()?;
    Ok(running)
}

impl RunningOperation {
    pub fn start(&mut self, step: StepKind) -> Result<(), Failure> {
        self.operation.start(step);
        self.save()?;
        Ok(())
    }

    pub fn finish(&mut self, step: StepKind) -> Result<(), Failure> {
        self.operation.finish(step);
        self.save()
    }

    pub fn finish_with_restore_paths(
        &mut self,
        step: StepKind,
        paths: Vec<String>,
    ) -> Result<(), Failure> {
        self.operation.finish_with_restore_paths(step, paths);
        self.save()
    }

    /// Операция дошла до конца: запись больше не нужна.
    pub fn complete(self) -> Result<(), Failure> {
        remove_record(&self.root)
    }

    /// Операция дошла до конца, но могла оставить рабочую директорию позади:
    /// запись убирается, только если за ней ничего не числится.
    pub fn complete_or_leave(self) -> Result<(), Failure> {
        match self.operation.resume() {
            Resume::Nothing => remove_record(&self.root),
            _ => Ok(()),
        }
    }

    /// Ждёт ли рабочая директория переливки за этой записью.
    pub fn leaves_restore(&self) -> bool {
        self.operation.resume() != Resume::Nothing
    }

    fn save(&self) -> Result<(), Failure> {
        let path = self.root.join(PATH);
        let tmp = path.with_extension("json.tmp");
        let mut file = File::create(&tmp)?;
        file.write_all(&self.operation.to_json()?)?;
        file.sync_all()?;
        fs::rename(tmp, path)?;
        Ok(())
    }
}

fn remove_record(root: &Path) -> Result<(), Failure> {
    match fs::remove_file(root.join(PATH)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Что осталось за прежней записью и как звалась её операция.
fn read_left(root: &Path) -> Result<Option<(&'static str, Resume)>, Failure> {
    let bytes = match fs::read(root.join(PATH)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    Ok(Some(match Operation::from_json(&bytes) {
        Ok(operation) => (operation.kind.name(), operation.resume()),
        Err(_) => ("operation", resume_unreadable()),
    }))
}

/// Перелить в рабочую директорию то, что осталось за прежней записью.
///
/// Вызывается под блокировкой: раз она наша, прежняя операция уже не идёт, и
/// её запись — всё, что от неё осталось. Оборвали её или она сама оставила
/// переливку следующему (sync), здесь неважно.
fn settle(root: &Path) -> Result<Option<RestoreOutcome>, Failure> {
    let Some((name, left)) = read_left(root)? else {
        return Ok(None);
    };
    let settled = match left {
        Resume::Nothing => Ok(None),
        Resume::RestoreLatest { paths } => {
            eprintln!("restoring what {name} left for the working tree");
            settle_latest(root, paths).map(Some)
        }
        Resume::RestoreVersion { version, then } => {
            eprintln!("finishing interrupted {name}: restoring version {version}");
            settle_version(root, &version).and_then(|restored| match *then {
                // Поверх оборванного restore прошёл sync: что он скачал, версия
                // не знает, и это переливается следом.
                Resume::RestoreLatest { paths } => settle_latest(root, paths).map(Some),
                _ => Ok(restored),
            })
        }
    };
    let settled = settled.map_err(|error| {
        format!(
            "cannot restore what {name} left: {error}\n\
             (if this can never succeed, `tresse abort` shows what would be given up)"
        )
    })?;
    remove_record(root)?;
    Ok(settled)
}

fn settle_latest(root: &Path, paths: Option<Vec<String>>) -> Result<RestoreOutcome, Failure> {
    let config = TresseConfig::load(root)?;
    let mut local = crate::cas::open_local_store(root)?;
    let paths = paths.map(|paths| paths.into_iter().collect());
    let outcome = crate::worktree::restore_latest(root, &config, &mut local, paths.as_ref())?;
    outcome.report_skipped();
    Ok(outcome)
}

/// Довести `restore <версия>`, оборванный до нас.
///
/// С охраной, хотя сам restore к версии её не знает: между обрывом и доводкой
/// могло пройти сколько угодно времени, и всё, что пользователь успел за него
/// изменить, просьбой перезаписать уже не покрыто.
fn settle_version(root: &Path, version: &str) -> Result<Option<RestoreOutcome>, Failure> {
    let config = TresseConfig::load(root)?;
    let mut local = crate::cas::open_local_store(root)?;
    let meta = crate::cas::load_merged_meta(&local)?;
    if crate::cas::resolve_version(&meta, version).is_err() {
        // Версии нет — значит, restore к ней ничего и не успел.
        return Ok(None);
    }
    let outcome = crate::worktree::restore_version(root, &config, &mut local, version, true)?;
    outcome.report_skipped();
    Ok(Some(outcome))
}
