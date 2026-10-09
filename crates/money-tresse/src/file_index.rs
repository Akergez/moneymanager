//! Файловая обвязка над общим индексом.
//!
//! Здесь только то, что знает про диск: как спросить у файловой системы время
//! и размер и где лежит сам индекс. Состав записи, правило сравнения и формат
//! файла принадлежат домену — [`tresse_lib::file_index`].

use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use tresse_lib::file_index::PATH;
use tresse_lib::{FileIndex, FileIndexEntry, IndexedFile};

use crate::restore_sink::WrittenFile;

pub fn metadata_entry(path: &Path, binary: bool) -> std::io::Result<FileIndexEntry> {
    let metadata = std::fs::metadata(path)?;
    let mtime_ms = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    Ok(FileIndexEntry {
        mtime_ms,
        size: metadata.len(),
        binary,
    })
}

/// Прочитать индекс, если он есть и читается.
///
/// Битый или отсутствующий индекс — не ошибка коммита: без него просто не будет
/// оптимизации, и все файлы прочитаются заново.
pub fn load(root: &Path) -> Option<FileIndex> {
    FileIndex::from_json(&std::fs::read(root.join(PATH)).ok()?).ok()
}

pub fn save(root: &Path, index: &FileIndex) -> Result<(), Box<dyn std::error::Error>> {
    let path = root.join(PATH);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, index.to_json()?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// Подновить индекс после восстановления файлов на диск.
///
/// Индекса могло ещё не быть — тогда он заводится здесь: по нему restore потом
/// узнаёт свои же файлы, и без записи о них реплика, которая только тянет,
/// приняла бы их за чужие.
///
/// Время и размер берутся те, с какими файл был записан, а не те, что на диске
/// сейчас: правка, сделанная сразу после записи, должна остаться заметной.
pub fn apply_restore(
    root: &Path,
    written: &[WrittenFile],
    removed: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if written.is_empty() && removed.is_empty() {
        return Ok(());
    }
    let mut index = load(root).unwrap_or_default();
    let mut restored = Vec::with_capacity(written.len());
    for file in written {
        // Блоб ещё не значит «бинарный»: текст без парсера тоже идёт блобом, а
        // природу файла индекс помнит такой, какой её увидел бы обход.
        let binary = file.blob
            && match index.get(&file.path) {
                Some(known) => known.binary,
                None => match is_utf8(&root.join(&file.path)) {
                    Ok(text) => !text,
                    Err(_) => continue,
                },
            };
        restored.push(IndexedFile::new(
            file.path.clone(),
            FileIndexEntry {
                mtime_ms: file.mtime_ms,
                size: file.size,
                binary,
            },
        ));
    }
    index.apply_restored(restored, removed.iter().map(String::as_str));
    save(root, &index)
}

/// Читается ли файл как UTF-8. Кусками: блоб целиком в память не поднимается.
fn is_utf8(path: &Path) -> std::io::Result<bool> {
    let mut file = std::fs::File::open(path)?;
    let mut chunk = vec![0u8; 64 * 1024];
    // Хвост символа, разрезанного границей куска.
    let mut pending: Vec<u8> = Vec::new();
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            return Ok(pending.is_empty());
        }
        pending.extend_from_slice(&chunk[..read]);
        match std::str::from_utf8(&pending) {
            Ok(_) => pending.clear(),
            Err(error) if error.error_len().is_some() => return Ok(false),
            Err(error) => {
                pending.drain(..error.valid_up_to());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_utf8;

    #[test]
    fn a_character_split_across_chunks_still_reads_as_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text");
        let mut text = "a".repeat(64 * 1024 - 1);
        text.push('ж');
        std::fs::write(&path, &text).unwrap();
        assert!(is_utf8(&path).unwrap());

        std::fs::write(&path, [b'a', 0xff, b'b']).unwrap();
        assert!(!is_utf8(&path).unwrap());

        std::fs::write(&path, &text.as_bytes()[..64 * 1024]).unwrap();
        assert!(!is_utf8(&path).unwrap());
    }
}
