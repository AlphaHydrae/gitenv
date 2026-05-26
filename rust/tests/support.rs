use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gitenv::{
    OperationAction, OperationEntry, OperationPlan, PlannedOperationAction, SourceAvailability,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryEntry {
    Directory { path: PathBuf },
    File { path: PathBuf, contents: String },
    Symlink { path: PathBuf, target: PathBuf },
}

pub fn directory(path: &str) -> DirectoryEntry {
    DirectoryEntry::Directory {
        path: PathBuf::from(path),
    }
}

pub fn file(path: &str, contents: &str) -> DirectoryEntry {
    DirectoryEntry::File {
        path: PathBuf::from(path),
        contents: contents.to_string(),
    }
}

#[cfg(unix)]
pub fn symlink_entry(path: &str, target: &Path) -> DirectoryEntry {
    DirectoryEntry::Symlink {
        path: PathBuf::from(path),
        target: target.to_path_buf(),
    }
}

#[allow(dead_code)]
pub fn available_operation_plan(actions: Vec<OperationAction>) -> OperationPlan {
    OperationPlan {
        entries: actions
            .into_iter()
            .map(|action| {
                OperationEntry::Action(PlannedOperationAction {
                    action,
                    source_availability: SourceAvailability::Available,
                    skip_reason: None,
                })
            })
            .collect(),
    }
}

pub fn snapshot_directory_contents(root: &Path) -> io::Result<Vec<DirectoryEntry>> {
    let mut entries = Vec::new();
    collect_directory_entries(root, root, &mut entries)?;
    entries.sort_by(|left, right| entry_path(left).cmp(entry_path(right)));
    Ok(entries)
}

fn entry_path(entry: &DirectoryEntry) -> &Path {
    match entry {
        DirectoryEntry::Directory { path }
        | DirectoryEntry::File { path, .. }
        | DirectoryEntry::Symlink { path, .. } => path,
    }
}

fn collect_directory_entries(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<DirectoryEntry>,
) -> io::Result<()> {
    let mut children = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.path());

    for child in children {
        let path = child.path();
        let relative_path = path
            .strip_prefix(root)
            .expect("collected path should remain inside snapshot root")
            .to_path_buf();
        let metadata = fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();

        if file_type.is_dir() {
            entries.push(DirectoryEntry::Directory {
                path: relative_path,
            });
            collect_directory_entries(root, &path, entries)?;
            continue;
        }

        if file_type.is_symlink() {
            entries.push(DirectoryEntry::Symlink {
                path: relative_path,
                target: fs::read_link(&path)?,
            });
            continue;
        }

        assert!(
            file_type.is_file(),
            "unexpected non-file entry in test fixture: {}",
            path.display()
        );

        entries.push(DirectoryEntry::File {
            path: relative_path,
            contents: fs::read_to_string(&path)?,
        });
    }

    Ok(())
}
