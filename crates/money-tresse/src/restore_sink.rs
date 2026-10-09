//! Выход restore на диск: куда и при каких условиях CLI кладёт файлы.
//!
//! Алгоритм restore живёт в домене и про диск не знает; здесь то, что знает:
//! запись через `.tresse/tmp/` с переименованием на место, охрана файлов,
//! изменённых пользователем, и учёт того, что на диске в итоге изменилось.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use tresse_lib::layout::{RESTORE_TEMP_DIR, TRESSE_DIR};
use tresse_lib::restore::RestoreSink;
use tresse_lib::{FileIndex, IgnoreRules};

use crate::file_index::metadata_entry;

/// Выход restore на диск.
///
/// С охраной ([`DiskRestoreSink::guarded`]) не трогает то, что пользователь
/// изменил после прошлого коммита или restore: такого содержимого нет в
/// истории, и затереть или удалить его значит потерять безвозвратно.
/// Пропущенное остаётся как есть.
///
/// Без охраны ([`DiskRestoreSink::forced`]) приводит диск к заказанному, что бы
/// на нём ни лежало, — это `tresse restore <версия>`, где перезапись и есть
/// просьба пользователя.
pub struct DiskRestoreSink {
    root: PathBuf,
    ignore: IgnoreRules,
    guard: Option<FileIndex>,
    staged: u64,
    written: Vec<WrittenFile>,
    removed: Vec<String>,
    skipped: Vec<String>,
}

/// Файл, который restore положил на диск, каким он был в момент записи.
///
/// Время и размер сняты до того, как файл встал на место: снятые позже, они
/// выдали бы правку, сделанную пользователем сразу после записи, за нетронутый
/// файл, и следующий restore её затёр бы.
pub struct WrittenFile {
    pub path: String,
    /// Пришёл ли файл блобом. Блоб ещё не значит «бинарный».
    pub blob: bool,
    pub mtime_ms: u64,
    pub size: u64,
}

/// Что restore сделал с диском на самом деле.
pub struct DiskChanges {
    pub written: Vec<WrittenFile>,
    pub removed: Vec<String>,
    /// Пути, которые охрана не дала тронуть.
    pub skipped: Vec<String>,
}

pub struct StagedBlob {
    path: PathBuf,
    file: File,
}

impl io::Write for StagedBlob {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut self.file, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut self.file)
    }
}

/// Что лежит на месте, куда restore собрался писать.
enum Occupant {
    /// Места никто не занял.
    Nobody,
    /// Файл, который Tresse сам положил и который с тех пор не меняли.
    Ours,
    /// Файл, которого Tresse не клал или который успели изменить.
    Foreign,
    /// Не файл: каталог там, где должен быть файл.
    Obstacle,
}

impl DiskRestoreSink {
    pub fn guarded(root: PathBuf, ignore: IgnoreRules, index: FileIndex) -> Self {
        Self::new(root, ignore, Some(index))
    }

    pub fn forced(root: PathBuf, ignore: IgnoreRules) -> Self {
        Self::new(root, ignore, None)
    }

    fn new(root: PathBuf, ignore: IgnoreRules, guard: Option<FileIndex>) -> Self {
        Self {
            root,
            ignore,
            guard,
            staged: 0,
            written: Vec::new(),
            removed: Vec::new(),
            skipped: Vec::new(),
        }
    }

    pub fn into_changes(self) -> DiskChanges {
        DiskChanges {
            written: self.written,
            removed: self.removed,
            skipped: self.skipped,
        }
    }

    /// Куда на диске ложится путь. `None` — путь закрыт правилами исключения.
    ///
    /// Закрытый путь restore обходит молча: коммит такой файл в форму не берёт,
    /// и положить его на диск значит завести то, чего репозиторий здесь не
    /// видит. Ошибкой это быть не может — запись дерева с таким путём законна,
    /// правила у реплик свои.
    fn target(&self, path: &str) -> io::Result<Option<PathBuf>> {
        let relative = Path::new(path);
        let plain = relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)));
        if path.is_empty() || !plain {
            return Err(io::Error::other(format!("unsafe restore path: {path}")));
        }
        // Весь путь, а не только первый его компонент: иначе restore кладёт на
        // диск то, что commit в форму не берёт.
        if self.ignore.is_ignored(path, false) {
            return Ok(None);
        }
        Ok(Some(self.root.join(relative)))
    }

    fn occupant(&self, path: &str, target: &Path) -> Occupant {
        let Some(index) = &self.guard else {
            return Occupant::Ours;
        };
        match fs::symlink_metadata(target) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Occupant::Nobody,
            Ok(metadata) if metadata.is_file() => match stat(target) {
                Ok((mtime_ms, size)) if index.holds(path, mtime_ms, size) => Occupant::Ours,
                _ => Occupant::Foreign,
            },
            _ => Occupant::Obstacle,
        }
    }

    /// Освободить место под файл: каталог на его пути или файл на месте каталога.
    ///
    /// Только без охраны. С охраной такая помеха останавливает операцию: молча
    /// обойти её нельзя — путь остался бы в проекции без файла на диске, и
    /// следующий коммит похоронил бы его.
    fn clear_obstacles(&self, target: &Path) -> io::Result<()> {
        if self.guard.is_some() {
            return Ok(());
        }
        if target.is_dir() {
            fs::remove_dir_all(target)?;
        }
        let mut ancestor = target.parent();
        while let Some(directory) = ancestor {
            if directory == self.root {
                break;
            }
            if directory.is_file() {
                fs::remove_file(directory)?;
            }
            ancestor = directory.parent();
        }
        Ok(())
    }

    fn stage_path(&mut self) -> io::Result<PathBuf> {
        let directory = self.root.join(TRESSE_DIR).join(RESTORE_TEMP_DIR);
        fs::create_dir_all(&directory)?;
        self.staged += 1;
        Ok(directory.join(format!("{}-{}", std::process::id(), self.staged)))
    }

    /// Поставить дописанный файл на место и запомнить, каким он туда встал.
    fn place(&mut self, path: &str, staged: &Path, target: &Path, blob: bool) -> io::Result<()> {
        let placed: io::Result<(u64, u64)> = (|| {
            let (mtime_ms, size) = stat(staged)?;
            self.clear_obstacles(target)?;
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(staged, target)?;
            Ok((mtime_ms, size))
        })();
        match placed {
            Ok((mtime_ms, size)) => {
                self.written.push(WrittenFile {
                    path: path.to_string(),
                    blob,
                    mtime_ms,
                    size,
                });
                Ok(())
            }
            Err(error) => {
                let _ = fs::remove_file(staged);
                Err(blocked(path, error))
            }
        }
    }

    /// Файл на месте уже тот, что нужен: писать нечего, но индекс его запомнит.
    fn accept_in_place(&mut self, path: &str, target: &Path, blob: bool) -> io::Result<()> {
        let (mtime_ms, size) = stat(target)?;
        self.written.push(WrittenFile {
            path: path.to_string(),
            blob,
            mtime_ms,
            size,
        });
        Ok(())
    }

    fn write_atomically(&mut self, path: &str, bytes: &[u8], blob: bool) -> io::Result<()> {
        let Some(target) = self.target(path)? else {
            return Ok(());
        };
        match self.occupant(path, &target) {
            Occupant::Obstacle => return Err(blocked(path, "a directory is in its place")),
            Occupant::Foreign => {
                // Совпало — так выглядит файл, записанный перед самым обрывом.
                if fs::read(&target).is_ok_and(|current| current == bytes) {
                    return self.accept_in_place(path, &target, blob);
                }
                self.skipped.push(path.to_string());
                return Ok(());
            }
            Occupant::Nobody | Occupant::Ours => {}
        }
        let staged = self.stage_path()?;
        fs::write(&staged, bytes)?;
        self.place(path, &staged, &target, blob)
    }
}

impl RestoreSink for DiskRestoreSink {
    type Blob = StagedBlob;

    fn write_text(&mut self, path: &str, content: &str) -> io::Result<()> {
        self.write_atomically(path, content.as_bytes(), false)
    }

    fn write_binary(&mut self, path: &str, data: &[u8]) -> io::Result<()> {
        self.write_atomically(path, data, true)
    }

    fn begin_blob(&mut self, path: &str) -> io::Result<Self::Blob> {
        self.target(path)?;
        let path = self.stage_path()?;
        let file = File::create(&path)?;
        Ok(StagedBlob { path, file })
    }

    fn finish_blob(&mut self, path: &str, mut blob: Self::Blob) -> io::Result<()> {
        io::Write::flush(&mut blob.file)?;
        drop(blob.file);
        let Some(target) = self.target(path)? else {
            let _ = fs::remove_file(&blob.path);
            return Ok(());
        };
        // Решается здесь, а не на входе: блоб пишется долго, и файл на месте
        // могли изменить, пока он шёл.
        match self.occupant(path, &target) {
            Occupant::Obstacle => {
                let _ = fs::remove_file(&blob.path);
                Err(blocked(path, "a directory is in its place"))
            }
            Occupant::Foreign => {
                let same = same_content(&blob.path, &target);
                let _ = fs::remove_file(&blob.path);
                if same {
                    return self.accept_in_place(path, &target, true);
                }
                self.skipped.push(path.to_string());
                Ok(())
            }
            Occupant::Nobody | Occupant::Ours => self.place(path, &blob.path, &target, true),
        }
    }

    fn remove(&mut self, path: &str) -> io::Result<()> {
        let Some(target) = self.target(path)? else {
            return Ok(());
        };
        match self.occupant(path, &target) {
            Occupant::Foreign => {
                self.skipped.push(path.to_string());
                return Ok(());
            }
            // Файла, который надо убрать, там и нет: на его месте каталог.
            Occupant::Obstacle => {}
            Occupant::Nobody | Occupant::Ours => match fs::remove_file(&target) {
                Ok(()) => self.prune_empty_parents(&target),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_) if target.is_dir() => fs::remove_dir_all(&target)?,
                Err(error) => return Err(error),
            },
        }
        self.removed.push(path.to_string());
        Ok(())
    }
}

impl DiskRestoreSink {
    /// Убрать каталоги, опустевшие после удаления файла.
    ///
    /// Пустой каталог Tresse не хранит, а оставленный на диске он занял бы место
    /// файла с тем же именем.
    fn prune_empty_parents(&self, target: &Path) {
        let mut ancestor = target.parent();
        while let Some(directory) = ancestor {
            if directory == self.root || fs::remove_dir(directory).is_err() {
                break;
            }
            ancestor = directory.parent();
        }
    }
}

fn stat(path: &Path) -> io::Result<(u64, u64)> {
    let entry = metadata_entry(path, false)?;
    Ok((entry.mtime_ms, entry.size))
}

/// Ошибка restore, который не смог положить файл туда, где ему место.
fn blocked(path: &str, reason: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!(
        "cannot restore {path}: {reason}; move whatever occupies its place and run the operation again"
    ))
}

/// Совпадают ли два файла побайтно. Любая ошибка чтения — «не совпадают».
fn same_content(left: &Path, right: &Path) -> bool {
    let sizes = (fs::metadata(left), fs::metadata(right));
    let (Ok(left_meta), Ok(right_meta)) = sizes else {
        return false;
    };
    if !right_meta.is_file() || left_meta.len() != right_meta.len() {
        return false;
    }
    let (Ok(mut left), Ok(mut right)) = (File::open(left), File::open(right)) else {
        return false;
    };
    let mut left_chunk = vec![0u8; 64 * 1024];
    let mut right_chunk = vec![0u8; 64 * 1024];
    loop {
        let Ok(read) = io::Read::read(&mut left, &mut left_chunk) else {
            return false;
        };
        if read == 0 {
            return true;
        }
        if io::Read::read_exact(&mut right, &mut right_chunk[..read]).is_err()
            || left_chunk[..read] != right_chunk[..read]
        {
            return false;
        }
    }
}
