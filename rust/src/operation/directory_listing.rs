use crate::boundary::{SourceReadError, SourceReadErrorKind};
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecursiveDirectoryEntryKind {
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryEntryRecord {
    name: String,
    kind: RecursiveDirectoryEntryKind,
}

enum DirectoryChildrenSource {
    Filesystem {
        path: PathBuf,
        read_dir: std::fs::ReadDir,
    },
    #[cfg(test)]
    Test {
        entries: VecDeque<Result<DirectoryEntryRecord, SourceReadError>>,
    },
}

impl DirectoryChildrenSource {
    fn filesystem(path: &Path) -> Result<Self, SourceReadError> {
        Ok(Self::Filesystem {
            path: path.to_path_buf(),
            read_dir: read_directory(path)?,
        })
    }

    #[cfg(test)]
    fn from_entries(
        entries: impl IntoIterator<Item = Result<DirectoryEntryRecord, SourceReadError>>,
    ) -> Self {
        Self::Test {
            entries: entries.into_iter().collect(),
        }
    }
}

impl Iterator for DirectoryChildrenSource {
    type Item = Result<DirectoryEntryRecord, SourceReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DirectoryChildrenSource::Filesystem { path, read_dir } => {
                let entry = read_dir.next()?;
                Some(read_directory_entry_record(path, entry))
            }
            #[cfg(test)]
            DirectoryChildrenSource::Test { entries } => entries.pop_front(),
        }
    }
}

fn read_directory_entry_record(
    path: &Path,
    entry: Result<std::fs::DirEntry, std::io::Error>,
) -> Result<DirectoryEntryRecord, SourceReadError> {
    let entry = read_dir_entry(path, entry)?;
    let file_type = read_entry_type(path, &entry);
    read_directory_entry_record_from_file_type(entry, file_type)
}

fn read_directory_entry_record_from_file_type(
    entry: std::fs::DirEntry,
    file_type: Result<std::fs::FileType, SourceReadError>,
) -> Result<DirectoryEntryRecord, SourceReadError> {
    let file_type = file_type?;

    Ok(DirectoryEntryRecord {
        name: entry.file_name().to_string_lossy().into_owned(),
        kind: if file_type.is_dir() {
            RecursiveDirectoryEntryKind::Directory
        } else {
            RecursiveDirectoryEntryKind::File
        },
    })
}

pub(super) fn list_directory_entries(
    path: &Path,
    max_depth: Option<usize>,
) -> Result<Vec<String>, SourceReadError> {
    let mut entries = Vec::new();
    collect_directory_entries(path, Path::new(""), max_depth, &mut entries)?;
    Ok(entries)
}

fn collect_directory_entries(
    directory: &Path,
    relative_prefix: &Path,
    remaining_depth: Option<usize>,
    entries: &mut Vec<String>,
) -> Result<(), SourceReadError> {
    let mut children = list_directory_children(directory)?;
    children.sort_by(|left, right| left.0.cmp(&right.0));

    for (name, kind) in children {
        let relative_path = relative_prefix.join(&name);
        let child_path = directory.join(&name);

        match kind {
            RecursiveDirectoryEntryKind::File => {
                entries.push(relative_path.to_string_lossy().into_owned());
            }
            RecursiveDirectoryEntryKind::Directory => {
                if can_descend(remaining_depth) {
                    let next_depth = decrement_depth(remaining_depth);
                    collect_directory_entries(&child_path, &relative_path, next_depth, entries)?;
                }
            }
        }
    }

    Ok(())
}

fn list_directory_children(
    path: &Path,
) -> Result<Vec<(String, RecursiveDirectoryEntryKind)>, SourceReadError> {
    let source = DirectoryChildrenSource::filesystem(path)?;
    list_directory_children_from_source(source)
}

fn list_directory_children_from_source(
    source: DirectoryChildrenSource,
) -> Result<Vec<(String, RecursiveDirectoryEntryKind)>, SourceReadError> {
    let mut entries = Vec::new();

    for entry in source {
        let entry = entry?;
        entries.push((entry.name, entry.kind));
    }

    Ok(entries)
}

fn can_descend(remaining_depth: Option<usize>) -> bool {
    match remaining_depth {
        None => true,
        Some(depth) => depth > 0,
    }
}

fn decrement_depth(remaining_depth: Option<usize>) -> Option<usize> {
    remaining_depth.map(|depth| depth.saturating_sub(1))
}

fn read_dir_entry(
    path: &Path,
    entry: Result<std::fs::DirEntry, std::io::Error>,
) -> Result<std::fs::DirEntry, SourceReadError> {
    entry.map_err(|error| source_read_error(path, error))
}

fn read_directory(path: &Path) -> Result<std::fs::ReadDir, SourceReadError> {
    std::fs::read_dir(path).map_err(|error| source_read_error(path, error))
}

fn read_entry_type(
    path: &Path,
    entry: &std::fs::DirEntry,
) -> Result<std::fs::FileType, SourceReadError> {
    map_entry_type_result(path, entry.file_type())
}

fn map_entry_type_result(
    path: &Path,
    file_type: Result<std::fs::FileType, std::io::Error>,
) -> Result<std::fs::FileType, SourceReadError> {
    file_type.map_err(|error| source_read_error(path, error))
}

fn source_read_error(path: &Path, error: std::io::Error) -> SourceReadError {
    let kind = match error.kind() {
        std::io::ErrorKind::NotFound => SourceReadErrorKind::Missing,
        std::io::ErrorKind::PermissionDenied => SourceReadErrorKind::PermissionDenied,
        _ => SourceReadErrorKind::UnexpectedIo,
    };

    SourceReadError {
        path: path.to_path_buf(),
        kind,
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DirectoryChildrenSource, DirectoryEntryRecord, RecursiveDirectoryEntryKind, can_descend,
        decrement_depth, list_directory_children, list_directory_children_from_source,
        list_directory_entries, map_entry_type_result, read_dir_entry, read_directory,
        read_directory_entry_record, read_directory_entry_record_from_file_type, source_read_error,
    };
    use crate::boundary::SourceReadErrorKind;
    use std::fs;
    use std::io::ErrorKind;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    #[test]
    fn list_directory_entries_returns_an_empty_list_for_an_empty_directory() {
        let root = TempDir::new().expect("temporary root should be created");

        let entries = list_directory_entries(root.path(), None)
            .expect("empty directories should list without error");

        assert!(entries.is_empty());
    }

    #[test]
    fn list_directory_children_reports_files_and_directories() {
        let root = TempDir::new().expect("temporary root should be created");
        let directory = root.path().join("configs");
        fs::create_dir_all(directory.join("nested")).expect("nested directory should be created");
        fs::write(directory.join("tool.conf"), "tool\n").expect("file should be written");

        let mut children = list_directory_children(&directory)
            .expect("directory children should be listed successfully");
        children.sort_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            children,
            vec![
                ("nested".to_string(), RecursiveDirectoryEntryKind::Directory),
                ("tool.conf".to_string(), RecursiveDirectoryEntryKind::File),
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn list_directory_children_treats_symlink_entries_as_files() {
        let root = TempDir::new().expect("temporary root should be created");
        let directory = root.path().join("configs");
        fs::create_dir_all(directory.join("nested")).expect("nested directory should be created");
        fs::write(directory.join("target.txt"), "target\n").expect("target file should be written");
        symlink(directory.join("nested"), directory.join("nested-link"))
            .expect("symlink entry should be created");

        let mut children = list_directory_children(&directory)
            .expect("directory children should be listed successfully");
        children.sort_by(|left, right| left.0.cmp(&right.0));

        assert_eq!(
            children,
            vec![
                ("nested".to_string(), RecursiveDirectoryEntryKind::Directory),
                ("nested-link".to_string(), RecursiveDirectoryEntryKind::File),
                ("target.txt".to_string(), RecursiveDirectoryEntryKind::File),
            ]
        );
    }

    #[test]
    fn list_directory_children_propagates_iterator_errors() {
        let error = source_read_error(
            Path::new("/repo-root/configs"),
            std::io::Error::new(ErrorKind::PermissionDenied, "iteration failed"),
        );

        let result = list_directory_children_from_source(DirectoryChildrenSource::from_entries([
            Ok(DirectoryEntryRecord {
                name: "tool.conf".to_string(),
                kind: RecursiveDirectoryEntryKind::File,
            }),
            Err(error.clone()),
        ]));

        let reported = result.expect_err("injected iterator errors should be preserved");
        assert_eq!(reported.path, error.path);
        assert_eq!(reported.kind, error.kind);
        assert_eq!(reported.message, error.message);
    }

    #[test]
    fn preserve_directory_open_errors_as_source_read_errors() {
        let root = TempDir::new().expect("temporary root should be created");
        let missing_directory = root.path().join("missing");

        let error = read_directory(&missing_directory)
            .expect_err("missing directory reads should be reported as errors");

        assert_eq!(error.path, missing_directory);
        assert_eq!(error.kind, SourceReadErrorKind::Missing);
    }

    #[test]
    fn preserve_directory_iteration_errors_as_source_read_errors() {
        let directory = PathBuf::from("/repo-root/configs");

        let error = read_dir_entry(
            &directory,
            Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "cannot iterate",
            )),
        )
        .expect_err("directory iteration failures should be preserved");

        assert_eq!(error.path, directory);
        assert_eq!(error.kind, SourceReadErrorKind::PermissionDenied);
        assert_eq!(error.message, "cannot iterate");
    }

    #[test]
    fn preserve_directory_entry_type_errors_as_source_read_errors() {
        let directory = PathBuf::from("/repo-root/configs");

        let error = map_entry_type_result(
            &directory,
            Err(std::io::Error::other("file type is unavailable")),
        )
        .expect_err("entry type failures should be preserved");

        assert_eq!(error.path, directory);
        assert_eq!(error.kind, SourceReadErrorKind::UnexpectedIo);
        assert_eq!(error.message, "file type is unavailable");
    }

    #[test]
    fn preserve_directory_entry_read_errors_as_source_read_errors() {
        let directory = PathBuf::from("/repo-root/configs");
        let entry_error =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "entry read failed");

        let error = read_dir_entry(&directory, Err(entry_error))
            .expect_err("directory entry read failures should be preserved");

        assert_eq!(error.path, directory);
        assert_eq!(error.kind, SourceReadErrorKind::PermissionDenied);
        assert_eq!(error.message, "entry read failed");
    }

    #[test]
    fn preserve_entry_record_read_success_from_the_shared_helper() {
        let root = TempDir::new().expect("temporary root should be created");
        let directory = root.path().join("configs");
        fs::create_dir_all(&directory).expect("directory should be created");
        fs::write(directory.join("tool.conf"), "tool\n").expect("file should be written");

        let mut children = std::fs::read_dir(&directory).expect("directory should read");
        let entry = children.next().expect("entry should exist");

        let record = read_directory_entry_record(&directory, entry)
            .expect("shared helper should build a directory entry record");

        assert_eq!(record.name, "tool.conf");
        assert_eq!(record.kind, RecursiveDirectoryEntryKind::File);
    }

    #[test]
    fn preserve_entry_record_directory_entry_read_errors_from_the_shared_helper() {
        let directory = PathBuf::from("/repo-root/configs");

        let error = read_directory_entry_record(
            &directory,
            Err(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "cannot iterate",
            )),
        )
        .expect_err("entry read failures should be preserved");

        assert_eq!(error.path, directory);
        assert_eq!(error.kind, SourceReadErrorKind::PermissionDenied);
        assert_eq!(error.message, "cannot iterate");
    }

    #[test]
    fn preserve_entry_record_file_type_errors_from_the_shared_helper() {
        let root = TempDir::new().expect("temporary root should be created");
        let directory = root.path().join("configs");
        fs::create_dir_all(&directory).expect("directory should be created");
        fs::write(directory.join("tool.conf"), "tool\n").expect("file should be written");

        let mut children = std::fs::read_dir(&directory).expect("directory should read");
        let entry = children
            .next()
            .expect("entry should exist")
            .expect("entry should be readable");

        let error = read_directory_entry_record_from_file_type(
            entry,
            Err(source_read_error(
                &directory,
                std::io::Error::other("file type is unavailable"),
            )),
        )
        .expect_err("entry type failures should be preserved");

        assert_eq!(error.path, directory);
        assert_eq!(error.kind, SourceReadErrorKind::UnexpectedIo);
        assert_eq!(error.message, "file type is unavailable");
    }

    #[test]
    fn map_recursive_source_read_errors_to_the_expected_availability_kinds() {
        let path = PathBuf::from("/repo-root/nested");

        let missing = source_read_error(
            &path,
            std::io::Error::new(ErrorKind::NotFound, "missing directory"),
        );
        let unexpected = source_read_error(&path, std::io::Error::other("boom"));

        assert_eq!(missing.path, path);
        assert_eq!(missing.kind, SourceReadErrorKind::Missing);
        assert_eq!(unexpected.path, path);
        assert_eq!(unexpected.kind, SourceReadErrorKind::UnexpectedIo);
    }

    #[test]
    fn decrement_depth_saturates_at_zero() {
        assert_eq!(decrement_depth(None), None);
        assert_eq!(decrement_depth(Some(2)), Some(1));
        assert_eq!(decrement_depth(Some(0)), Some(0));
    }

    #[test]
    fn can_descend_respects_remaining_depth() {
        assert!(can_descend(None));
        assert!(can_descend(Some(1)));
        assert!(!can_descend(Some(0)));
    }

    #[test]
    fn list_directory_entries_orders_nested_entries_depth_first() {
        let root = TempDir::new().expect("temporary root should be created");
        let nested = root.path().join("nested");
        fs::create_dir_all(nested.join("deeper")).expect("nested directory should be created");
        fs::write(root.path().join("top.txt"), "top\n").expect("top file should be written");
        fs::write(nested.join("inner.txt"), "inner\n").expect("inner file should be written");
        fs::write(nested.join("deeper").join("deep.txt"), "deep\n")
            .expect("deep file should be written");

        let entries = list_directory_entries(root.path(), None)
            .expect("recursive directory listing should succeed");

        assert_eq!(
            entries,
            vec![
                "nested/deeper/deep.txt".to_string(),
                "nested/inner.txt".to_string(),
                "top.txt".to_string(),
            ]
        );
    }

    #[test]
    fn list_directory_entries_stops_at_depth_zero() {
        let root = TempDir::new().expect("temporary root should be created");
        fs::create_dir_all(root.path().join("nested")).expect("nested directory should be created");
        fs::write(root.path().join("nested").join("inner.txt"), "inner\n")
            .expect("inner file should be written");
        fs::write(root.path().join("top.txt"), "top\n").expect("top file should be written");

        let entries = list_directory_entries(root.path(), Some(0))
            .expect("shallow directory listing should succeed");

        assert_eq!(entries, vec!["top.txt".to_string()]);
    }
}
