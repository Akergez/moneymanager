use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

use crate::file_index::metadata_entry;
use tresse_lib::file_index::shape_from_files;
use tresse_lib::{FileIndex, FlatFileSource, IgnoreRules, IndexedFile, Shape};

// ---------------------------------------------------------------------------
// Shape builder (path structure only, no content loaded)
// ---------------------------------------------------------------------------

/// Обойти рабочую директорию и отдать домену список файлов с их метаданными.
///
/// Здесь только внешний мир: обход, правила исключения и определение природы
/// файла. Что из этого следует — какой путь считать нетронутым и как сложить
/// форму — решает [`tresse_lib::file_index::shape_from_files`].
pub fn build_shape(
    root: &Path,
    ignore: &IgnoreRules,
    previous: Option<&FileIndex>,
) -> std::io::Result<(Shape, FileIndex)> {
    let mut files = Vec::new();
    collect_files(root, "", ignore, previous, &mut files)?;
    Ok(shape_from_files(files, previous))
}

fn collect_files(
    abs: &Path,
    rel: &str,
    ignore: &IgnoreRules,
    previous: Option<&FileIndex>,
    files: &mut Vec<IndexedFile>,
) -> std::io::Result<()> {
    if abs.is_file() {
        // Природу файла определяем один раз и запоминаем: перечитывать его
        // целиком ради этого на каждом коммите незачем.
        let binary = previous.and_then(|index| index.get(rel)).map_or_else(
            || std::fs::read_to_string(abs).is_err(),
            |entry| entry.binary,
        );
        files.push(IndexedFile::new(rel, metadata_entry(abs, binary)?));
        return Ok(());
    }

    let mut entries: Vec<_> = std::fs::read_dir(abs)?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let child_rel = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if ignore.is_ignored(&child_rel, entry.path().is_dir()) {
            continue;
        }
        if let Err(error) = collect_files(&entry.path(), &child_rel, ignore, previous, files) {
            eprintln!("  skipping {}: {error}", entry.path().display());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// FlatFileSource backed by disk
// ---------------------------------------------------------------------------

pub struct DiskSource(pub PathBuf);

#[cfg(test)]
mod index_tests {
    use super::*;

    #[test]
    fn missing_index_scans_all_files_then_matching_index_marks_them_unchanged() {
        let root = std::env::temp_dir().join(format!("tresse-index-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.md"), "hello").unwrap();

        let (first, index) = build_shape(&root, &IgnoreRules::default(), None).unwrap();
        let Shape::Dir { children, .. } = first else {
            panic!()
        };
        assert!(matches!(children.as_slice(), [Shape::File { path }] if path == "a.md"));

        let (second, _) = build_shape(&root, &IgnoreRules::default(), Some(&index)).unwrap();
        let Shape::Dir { children, .. } = second else {
            panic!()
        };
        assert!(
            matches!(children.as_slice(), [Shape::Unchanged { path, binary: false }] if path == "a.md")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

impl FlatFileSource for DiskSource {
    fn len(&self, path: &str) -> u64 {
        std::fs::metadata(self.0.join(path))
            .map(|m| m.len())
            .unwrap_or(0)
    }

    fn read(&self, path: &str, offset: u64, len: usize) -> Vec<u8> {
        let Ok(mut f) = std::fs::File::open(self.0.join(path)) else {
            return Vec::new();
        };
        if f.seek(std::io::SeekFrom::Start(offset)).is_err() {
            return Vec::new();
        }
        let mut buf = vec![0u8; len];
        let n = f.read(&mut buf).unwrap_or(0);
        buf.truncate(n);
        buf
    }
}

#[cfg(test)]
mod tests {
    use tresse_lib::IgnoreRules;

    #[test]
    fn tresse_directory_is_always_ignored() {
        assert!(IgnoreRules::default().is_ignored(".tresse", true));
    }
}
