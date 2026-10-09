//! Удалённое хранилище в каталоге на этой же машине — только для тестов.
//!
//! Тесты запускают настоящий `tresse` на двух репозиториях, и обменяться
//! объектами без сети им больше негде. Включается переменной окружения (см.
//! `ConfiguredRemote::open`), в конфиге его нет.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rdx_sync::{RemoteKv, SyncError};

/// Тот же недоверенный KV, что S3 и сервер: ключ — путь файла внутри каталога,
/// значения лежат зашифрованными, имена ослеплены.
pub struct DirRemote(PathBuf);

impl DirRemote {
    pub fn open(directory: impl Into<PathBuf>) -> Self {
        Self(directory.into())
    }
}

impl DirRemote {
    fn file(&self, key: &str) -> Result<PathBuf, SyncError> {
        let relative = Path::new(key);
        let plain = relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)));
        if key.is_empty() || !plain {
            return Err(SyncError::Other(format!("unsafe remote key: {key}")));
        }
        Ok(self.0.join(relative))
    }

    fn collect(&self, directory: &Path, prefix: &str, keys: &mut Vec<String>) -> io::Result<()> {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let key = format!("{prefix}{name}");
            if entry.file_type()?.is_dir() {
                self.collect(&entry.path(), &format!("{key}/"), keys)?;
            } else if !name.ends_with(".tmp") {
                keys.push(key);
            }
        }
        Ok(())
    }
}

impl RemoteKv for DirRemote {
    fn list(&self, prefix: &str) -> Result<Vec<String>, SyncError> {
        let mut keys = Vec::new();
        self.collect(&self.0, "", &mut keys)?;
        keys.retain(|key| key.starts_with(prefix));
        Ok(keys)
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, SyncError> {
        match fs::read(self.file(key)?) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Err(SyncError::NotFound(key.to_string()))
            }
            Err(error) => Err(error.into()),
        }
    }

    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        let path = self.file(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, value)?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<(), SyncError> {
        match fs::remove_file(self.file(key)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}
