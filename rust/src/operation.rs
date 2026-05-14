//! Operation planning stage.
//!
//! Converts an [`IntentPlan`] into a fully resolved [`OperationPlan`] by
//! expanding source-relative paths and selector patterns into concrete
//! filesystem operations. This stage only plans — no filesystem writes happen
//! here.
//!
//! The composition root in `lib.rs` wires a real [`DirectoryEntriesReader`]
//! boundary into an [`OperationContext`]. Tests supply a lightweight local
//! double that returns pre-built directory listings without touching the disk.

use crate::config::SelectionType;
use crate::{
    ActionMode, ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
    ProgramError, ResolvedOptions, SourceAvailability, SourceUnreadableKind,
    boundary::{
        DirectoryEntriesReader, SourceAvailabilityReader, SourcePathRequirement, SourceReadError,
        SourceReadErrorKind,
    },
    logging, path_resolution,
};
use log::Level;
use std::path::{Path, PathBuf};

/// Concrete, execution-shaped planning output.
///
/// Unlike `IntentPlan`, this stage resolves repository-relative source roots
/// and home-relative target directories into explicit filesystem operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationPlan {
    /// Ordered list of concrete operations and planning issues.
    pub entries: Vec<OperationEntry>,
}

impl OperationPlan {
    pub(crate) fn apply_blocking_diagnostics(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                OperationEntry::Action(action) => action
                    .source_availability
                    .diagnostic_suffix()
                    .map(|suffix| {
                        format!(
                            "source {} for {} {suffix}",
                            action.action.source().display(),
                            action.action.target().display(),
                        )
                    }),
                OperationEntry::Issue(issue) => issue
                    .source_availability
                    .diagnostic_suffix()
                    .map(|suffix| format!("source root {} {suffix}", issue.path.display())),
            })
            .collect()
    }
}

/// Ordered operation-plan entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationEntry {
    Action(PlannedOperationAction),
    Issue(OperationPlanningIssue),
}

/// Concrete action entry plus the resolved readability of its source path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedOperationAction {
    pub action: OperationAction,
    pub source_availability: SourceAvailability,
}

/// Planning issue that blocks selector expansion at a specific config position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationPlanningIssue {
    pub path: PathBuf,
    pub source_availability: SourceAvailability,
}

/// A concrete operation ready for status inspection or filesystem execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationAction {
    Copy(FileOperation),
    Symlink(FileOperation),
}

impl OperationAction {
    pub(crate) fn source(&self) -> &Path {
        match self {
            OperationAction::Copy(operation) | OperationAction::Symlink(operation) => {
                operation.source.as_path()
            }
        }
    }

    pub(crate) fn target(&self) -> &Path {
        match self {
            OperationAction::Copy(operation) | OperationAction::Symlink(operation) => {
                operation.target.as_path()
            }
        }
    }
}

/// A concrete file operation with fully resolved source and target paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOperation {
    pub source: PathBuf,
    pub target: PathBuf,
    pub mkdir: bool,
    pub conflict_policy: ConflictPolicy,
}

/// Stage-owned context for operation planning, carrying resolved runtime state
/// and trait-backed dependency references.
///
/// The composition root creates a `RealBoundary`-backed context; tests create
/// lightweight local doubles so no real filesystem access is needed.
pub(crate) struct OperationContext<'a> {
    /// Resolved home directory for the current process invocation.
    /// Carried as data rather than as a trait method so helper signatures
    /// stay small and the context fully represents stage runtime inputs.
    pub(crate) home_directory: PathBuf,
    /// Boundary adapter for listing directory entries (used for selector expansion).
    pub(crate) dir_reader: &'a dyn DirectoryEntriesReader,
    /// Boundary adapter for probing direct source path availability.
    pub(crate) source_reader: &'a dyn SourceAvailabilityReader,
    /// Platform- or configuration-derived exclusions applied to every selector.
    pub(crate) global_selection_excludes: Vec<String>,
}

/// Internal planning function accepting a stage-owned context for
/// deterministic tests and composition-root injection.
pub(crate) fn derive_operation_plan(
    intent_plan: &IntentPlan,
    context: &OperationContext,
) -> Result<OperationPlan, ProgramError> {
    logging::operation(
        Level::Debug,
        "operation_plan_start",
        format!(
            "repository={} sources={}",
            intent_plan.repository,
            intent_plan.sources.len()
        ),
    );

    let repository_root = resolve_repository_root(&intent_plan.repository, &context.home_directory);
    let mut entries = Vec::new();

    for source in &intent_plan.sources {
        let source_root =
            resolve_source_root(&repository_root, &source.from, &context.home_directory);

        for action in &source.actions {
            match action {
                IntentAction::File(file_action) => {
                    entries.push(expand_file_action(
                        &source_root,
                        file_action,
                        &context.home_directory,
                        context.source_reader,
                    ));
                }
                IntentAction::Select(select_action) => {
                    entries.extend(expand_select_action(
                        &source_root,
                        select_action,
                        &context.home_directory,
                        &context.global_selection_excludes,
                        context.source_reader,
                        &|path| context.dir_reader.list_directory_entries(path),
                    ));
                }
            }
        }
    }

    logging::operation(
        Level::Info,
        "operation_plan_success",
        format!("entries={}", entries.len()),
    );

    Ok(OperationPlan { entries })
}

fn resolve_repository_root(repository: &str, home_directory: &Path) -> PathBuf {
    path_resolution::expand_home_prefixed_or_literal_path(repository, home_directory)
}

fn resolve_source_root(
    repository_root: &Path,
    source_from: &str,
    home_directory: &Path,
) -> PathBuf {
    let path = Path::new(source_from);
    if path.is_absolute() || source_from == "~" || source_from.starts_with("~/") {
        path_resolution::expand_home_prefixed_or_literal_path(source_from, home_directory)
    } else {
        repository_root.join(path)
    }
}

fn resolve_target_directory(target_directory: &str, home_directory: &Path) -> PathBuf {
    let path = Path::new(target_directory);
    if path.is_absolute() || target_directory == "~" || target_directory.starts_with("~/") {
        path_resolution::expand_home_prefixed_or_literal_path(target_directory, home_directory)
    } else {
        home_directory.join(path)
    }
}

fn expand_file_action(
    source_root: &Path,
    file_action: &IntentFileAction,
    home_directory: &Path,
    source_reader: &dyn SourceAvailabilityReader,
) -> OperationEntry {
    let action = operation_action(
        source_root,
        &file_action.file,
        &file_action.as_name,
        &file_action.options,
        home_directory,
    );

    OperationEntry::Action(PlannedOperationAction {
        source_availability: source_availability_from_result(
            source_reader.ensure_source_path_readable(action.source(), SourcePathRequirement::File),
        ),
        action,
    })
}

fn expand_select_action(
    source_root: &Path,
    select_action: &IntentSelectAction,
    home_directory: &Path,
    global_selection_excludes: &[String],
    source_reader: &dyn SourceAvailabilityReader,
    list_directory: &impl Fn(&Path) -> Result<Vec<String>, SourceReadError>,
) -> Vec<OperationEntry> {
    let source_root_availability = source_availability_from_result(
        source_reader.ensure_source_path_readable(source_root, SourcePathRequirement::Directory),
    );

    if source_root_availability != SourceAvailability::Available {
        return vec![OperationEntry::Issue(OperationPlanningIssue {
            path: source_root.to_path_buf(),
            source_availability: source_root_availability,
        })];
    }

    // TODO(increment-61): when `select.recursive` is true, traverse nested
    // source directories and keep descendant-relative target mapping.
    // Closure condition: remove this TODO once recursive operation expansion and
    // its integration coverage are implemented.
    let entries = match list_directory(source_root) {
        Ok(entries) => entries,
        Err(error) => {
            let source_availability = source_availability_from_error(&error);
            let path = error.path;
            return vec![OperationEntry::Issue(OperationPlanningIssue {
                path,
                source_availability,
            })];
        }
    };
    let total_entries = entries.len();
    let selected_entries = entries
        .into_iter()
        .filter(|entry| {
            should_include_selection_entry(entry, select_action, global_selection_excludes)
        })
        .collect::<Vec<_>>();

    logging::operation(
        Level::Debug,
        "expand_select_action",
        format!(
            "source_root={} entries={} selected={}",
            source_root.display(),
            total_entries,
            selected_entries.len()
        ),
    );

    selected_entries
        .into_iter()
        .map(|entry| {
            let action = operation_action(
                source_root,
                &entry,
                &entry,
                &select_action.options,
                home_directory,
            );

            OperationEntry::Action(PlannedOperationAction {
                source_availability: source_availability_from_result(
                    source_reader
                        .ensure_source_path_readable(action.source(), SourcePathRequirement::File),
                ),
                action,
            })
        })
        .collect()
}

fn source_availability_from_result(result: Result<(), SourceReadError>) -> SourceAvailability {
    match result {
        Ok(()) => SourceAvailability::Available,
        Err(error) => source_availability_from_error(&error),
    }
}

fn source_availability_from_error(error: &SourceReadError) -> SourceAvailability {
    match error.kind {
        SourceReadErrorKind::Missing => SourceAvailability::Missing,
        SourceReadErrorKind::PermissionDenied => SourceAvailability::Unreadable {
            kind: SourceUnreadableKind::PermissionDenied,
            message: error.message.clone(),
        },
        SourceReadErrorKind::UnexpectedIo => SourceAvailability::Unreadable {
            kind: SourceUnreadableKind::UnexpectedIo,
            message: error.message.clone(),
        },
    }
}

impl SourceAvailability {
    pub(crate) fn diagnostic_suffix(&self) -> Option<String> {
        match self {
            SourceAvailability::Available => None,
            SourceAvailability::Missing => Some("is missing".to_string()),
            SourceAvailability::Unreadable { message, .. } => {
                Some(format!("is unreadable ({message})"))
            }
        }
    }
}

fn operation_action(
    source_root: &Path,
    source_file: &str,
    target_file_name: &str,
    options: &ResolvedOptions,
    home_directory: &Path,
) -> OperationAction {
    let file_operation = FileOperation {
        source: source_root.join(source_file),
        target: resolve_target_directory(&options.to, home_directory).join(target_file_name),
        mkdir: options.mkdir,
        conflict_policy: options.conflict_policy.clone(),
    };

    match options.mode {
        ActionMode::Copy => OperationAction::Copy(file_operation),
        ActionMode::Symlink => OperationAction::Symlink(file_operation),
    }
}

fn should_include_selection_entry(
    entry: &str,
    select_action: &IntentSelectAction,
    global_selection_excludes: &[String],
) -> bool {
    let has_dot_prefix = entry.starts_with('.');
    let selected_by_type = match select_action.selection_type {
        SelectionType::Dot => has_dot_prefix,
        SelectionType::NonDot => !has_dot_prefix,
        SelectionType::All => true,
    };

    selected_by_type
        && !global_selection_excludes
            .iter()
            .any(|excluded| excluded == entry)
        && !select_action
            .exclude
            .iter()
            .any(|excluded| excluded == entry)
}

#[cfg(test)]
mod tests {
    use super::{
        FileOperation, OperationAction, OperationContext, OperationEntry, OperationPlan,
        OperationPlanningIssue, PlannedOperationAction, SourceReadErrorKind, derive_operation_plan,
    };
    use crate::{
        ActionMode, ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
        IntentSource, ResolvedOptions, SelectionType, SourceAvailability, SourceUnreadableKind,
        boundary::{
            DirectoryEntriesReader, SourcePathRequirement, SourceReadError,
            test_doubles::{FnDirectoryReader, FnSourceAvailabilityReader},
        },
        derive_operation_plan as derive_operation_plan_entrypoint,
    };
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    // ---------------------------------------------------------------------------
    // Intent model constructors
    // ---------------------------------------------------------------------------

    fn make_intent_plan(repository: &Path, sources: Vec<IntentSource>) -> IntentPlan {
        IntentPlan {
            repository: repository.to_string_lossy().into_owned(),
            sources,
        }
    }

    fn make_intent_source(from: &str, actions: Vec<IntentAction>) -> IntentSource {
        IntentSource {
            from: from.to_string(),
            actions,
        }
    }

    fn make_file_action(file: &str, as_name: &str, options: ResolvedOptions) -> IntentAction {
        IntentAction::File(IntentFileAction {
            file: file.to_string(),
            as_name: as_name.to_string(),
            options,
        })
    }

    fn make_select_action(
        selection_type: SelectionType,
        exclude: Vec<&str>,
        options: ResolvedOptions,
    ) -> IntentAction {
        IntentAction::Select(IntentSelectAction {
            selection_type,
            recursive: false,
            exclude: exclude.into_iter().map(str::to_string).collect(),
            options,
        })
    }

    fn default_options() -> ResolvedOptions {
        ResolvedOptions {
            mode: ActionMode::Symlink,
            to: "~".to_string(),
            mkdir: true,
            conflict_policy: ConflictPolicy::Skip,
        }
    }

    type ReadableSourceReader =
        FnSourceAvailabilityReader<fn(&Path, SourcePathRequirement) -> Result<(), SourceReadError>>;

    static READABLE_SOURCE_READER: ReadableSourceReader =
        FnSourceAvailabilityReader(readable_source_path);

    fn make_operation_context<'a>(
        home_directory: PathBuf,
        dir_reader: &'a dyn DirectoryEntriesReader,
        global_selection_excludes: Option<Vec<&str>>,
    ) -> OperationContext<'a> {
        OperationContext {
            home_directory,
            dir_reader,
            source_reader: &READABLE_SOURCE_READER,
            global_selection_excludes: global_selection_excludes
                .unwrap_or_default()
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }

    // ---------------------------------------------------------------------------
    // Operation plan constructors
    // ---------------------------------------------------------------------------

    fn expected_operation_plan(actions: Vec<OperationAction>) -> OperationPlan {
        OperationPlan {
            entries: actions
                .into_iter()
                .map(|action| {
                    OperationEntry::Action(PlannedOperationAction {
                        action,
                        source_availability: SourceAvailability::Available,
                    })
                })
                .collect(),
        }
    }

    fn expected_symlink(source: PathBuf, target: PathBuf) -> OperationAction {
        OperationAction::Symlink(FileOperation {
            source,
            target,
            mkdir: true,
            conflict_policy: ConflictPolicy::Skip,
        })
    }

    fn expected_copy(source: PathBuf, target: PathBuf) -> OperationAction {
        OperationAction::Copy(FileOperation {
            source,
            target,
            mkdir: true,
            conflict_policy: ConflictPolicy::Skip,
        })
    }

    fn readable_source_path(
        _path: &Path,
        _requirement: SourcePathRequirement,
    ) -> Result<(), SourceReadError> {
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    #[test]
    fn expand_dot_selection_entries_into_concrete_file_actions() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".zshrc"), "export TEST=1\n")
            .expect("dotfile should be written");
        fs::write(repository.path().join(".gitconfig"), "[user]\n")
            .expect("excluded dotfile should be written");
        fs::write(repository.path().join("notes.txt"), "plain file\n")
            .expect("non-dotfile should be written");
        fs::create_dir_all(repository.path().join(".ssh"))
            .expect("dot-directory should be created");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![".gitconfig"],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand selected files");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }

    #[test]
    fn keep_intent_file_actions_as_concrete_file_actions() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_file_action(".zshrc", ".zshrc", default_options())],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should keep explicit file actions");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.join("configs").join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }

    #[test]
    fn expand_dot_selection_entries_into_sorted_actions() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".z-last"), "a\n").expect("dotfile should be written");
        fs::write(repository.path().join(".a-first"), "b\n").expect("dotfile should be written");
        fs::write(repository.path().join(".m-middle"), "c\n").expect("dotfile should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|path| {
            let path = path.to_path_buf();
            crate::fs_adapter::list_directory_entries(&path)
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand selected files");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.path().join(".a-first"),
                    home.path().join(".a-first"),
                ),
                expected_symlink(
                    repository.path().join(".m-middle"),
                    home.path().join(".m-middle"),
                ),
                expected_symlink(
                    repository.path().join(".z-last"),
                    home.path().join(".z-last"),
                ),
            ])
        );
    }

    #[cfg(unix)]
    #[test]
    fn include_dotfile_symlink_entries_in_selector_expansion() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join("target.txt"), "target\n")
            .expect("target file should be written");
        symlink(
            repository.path().join("target.txt"),
            repository.path().join(".linked"),
        )
        .expect("dotfile symlink should be created");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|path| {
            let path = path.to_path_buf();
            crate::fs_adapter::list_directory_entries(&path)
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand selected symlink entries");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".linked"),
                home.path().join(".linked"),
            )])
        );
    }

    #[test]
    fn expand_non_dot_selection_entries_when_type_is_non_dot() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::NonDot,
                    vec!["notes.txt"],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|_| {
            Ok(vec![
                ".zshrc".to_string(),
                "notes.txt".to_string(),
                "tmux.conf".to_string(),
            ])
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand non-dotfile selection");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.join("configs").join("tmux.conf"),
                home.path().join("tmux.conf"),
            )])
        );
    }

    #[test]
    fn expand_all_selection_entries_when_type_is_all() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::All,
                    vec!["notes.txt"],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|_| {
            Ok(vec![
                ".zshrc".to_string(),
                "notes.txt".to_string(),
                "tmux.conf".to_string(),
            ])
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand all selection entries");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.join("configs").join(".zshrc"),
                    home.path().join(".zshrc")
                ),
                expected_symlink(
                    repository.join("configs").join("tmux.conf"),
                    home.path().join("tmux.conf"),
                ),
            ])
        );
    }

    #[test]
    fn cannot_derive_operation_plan_when_directory_listing_fails() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|path| {
            Err(SourceReadError {
                path: path.to_path_buf(),
                kind: crate::boundary::SourceReadErrorKind::UnexpectedIo,
                message: "boom".to_string(),
            })
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("selector expansion should preserve planning issues");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Issue(OperationPlanningIssue {
                    path: PathBuf::from("/repo-root/configs"),
                    source_availability: SourceAvailability::Unreadable {
                        kind: SourceUnreadableKind::UnexpectedIo,
                        message: "boom".to_string(),
                    },
                })],
            }
        );
    }

    #[test]
    fn preserve_a_missing_source_root_as_a_planning_issue() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|_| Ok(vec![]));
        let source_reader = FnSourceAvailabilityReader(|path, _| {
            Err(SourceReadError {
                path: path.to_path_buf(),
                kind: SourceReadErrorKind::Missing,
                message: "missing".to_string(),
            })
        });
        let context = OperationContext {
            home_directory: home.path().to_path_buf(),
            dir_reader: &dir_reader,
            source_reader: &source_reader,
            global_selection_excludes: vec![],
        };

        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("missing source roots should be preserved as planning issues");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Issue(OperationPlanningIssue {
                    path: repository.join("configs"),
                    source_availability: SourceAvailability::Missing,
                })],
            }
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let source_reader = FnSourceAvailabilityReader(|_, _| Ok(()));
        let available_context = OperationContext {
            home_directory: home.path().to_path_buf(),
            dir_reader: &dir_reader,
            source_reader: &source_reader,
            global_selection_excludes: vec![],
        };
        let available_plan = derive_operation_plan(
            &make_intent_plan(
                &repository,
                vec![make_intent_source(
                    "configs",
                    vec![make_file_action(".zshrc", ".zshrc", default_options())],
                )],
            ),
            &available_context,
        )
        .expect("available file sources should be preserved as actions");

        assert_eq!(
            available_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.join("configs").join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }

    #[test]
    fn mark_a_file_action_source_as_unreadable_when_permission_is_denied() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");

        fs::write(repository.path().join(".zshrc"), "export TEST=1\n")
            .expect("source file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_file_action(".zshrc", ".zshrc", default_options())],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let source_reader = FnSourceAvailabilityReader(|path, _| {
            Err(SourceReadError {
                path: path.to_path_buf(),
                kind: SourceReadErrorKind::PermissionDenied,
                message: "denied".to_string(),
            })
        });
        let context = OperationContext {
            home_directory: home.path().to_path_buf(),
            dir_reader: &dir_reader,
            source_reader: &source_reader,
            global_selection_excludes: vec![],
        };

        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("permission-denied file sources should be preserved in the plan");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Action(PlannedOperationAction {
                    action: expected_symlink(
                        repository.path().join(".").join(".zshrc"),
                        home.path().join(".zshrc"),
                    ),
                    source_availability: SourceAvailability::Unreadable {
                        kind: SourceUnreadableKind::PermissionDenied,
                        message: "denied".to_string(),
                    },
                })],
            }
        );

        let dir_reader = FnDirectoryReader(|_| Ok(vec![".zshrc".to_string()]));
        let readable_source_reader =
            FnSourceAvailabilityReader(|_, requirement| match requirement {
                SourcePathRequirement::Directory => Ok(()),
                SourcePathRequirement::File => Ok(()),
            });
        let readable_context = OperationContext {
            home_directory: home.path().to_path_buf(),
            dir_reader: &dir_reader,
            source_reader: &readable_source_reader,
            global_selection_excludes: vec![],
        };
        let readable_plan = derive_operation_plan(
            &make_intent_plan(
                repository.path(),
                vec![make_intent_source(
                    ".",
                    vec![make_select_action(
                        SelectionType::Dot,
                        vec![],
                        default_options(),
                    )],
                )],
            ),
            &readable_context,
        )
        .expect("available directories should expand into actions");

        assert_eq!(
            readable_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }

    #[test]
    fn source_reader_double_can_return_file_ok_and_directory_error() {
        let repository = PathBuf::from("/repo-root");
        let source_reader = FnSourceAvailabilityReader(|_, requirement| match requirement {
            SourcePathRequirement::Directory => Err(SourceReadError {
                path: repository.join("configs"),
                kind: SourceReadErrorKind::Missing,
                message: "missing".to_string(),
            }),
            SourcePathRequirement::File => Ok(()),
        });

        assert!(
            crate::boundary::SourceAvailabilityReader::ensure_source_path_readable(
                &source_reader,
                &repository.join("configs").join(".zshrc"),
                SourcePathRequirement::File,
            )
            .is_ok()
        );
        assert!(
            crate::boundary::SourceAvailabilityReader::ensure_source_path_readable(
                &source_reader,
                &repository.join("configs"),
                SourcePathRequirement::Directory,
            )
            .is_err()
        );
    }

    #[test]
    fn source_reader_double_can_return_directory_ok_and_file_error() {
        let source_reader = FnSourceAvailabilityReader(|path, requirement| match requirement {
            SourcePathRequirement::File => Err(SourceReadError {
                path: path.to_path_buf(),
                kind: SourceReadErrorKind::PermissionDenied,
                message: "denied".to_string(),
            }),
            SourcePathRequirement::Directory => Ok(()),
        });

        assert!(
            crate::boundary::SourceAvailabilityReader::ensure_source_path_readable(
                &source_reader,
                Path::new("/repo-root"),
                SourcePathRequirement::Directory,
            )
            .is_ok()
        );
        assert!(
            crate::boundary::SourceAvailabilityReader::ensure_source_path_readable(
                &source_reader,
                Path::new("/repo-root/.zshrc"),
                SourcePathRequirement::File,
            )
            .is_err()
        );
    }

    #[test]
    fn resolve_relative_targets_against_the_home_directory() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");
        let mut options = default_options();
        options.to = "links".to_string();

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_file_action(".zshrc", "shell/zshrc", options)],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should anchor relative targets to the home directory");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.join("configs").join(".zshrc"),
                home.path().join("links").join("shell/zshrc"),
            )])
        );
    }

    #[test]
    fn resolve_home_prefixed_sources_and_targets_for_copy_operations() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");
        let mut options = default_options();
        options.mode = ActionMode::Copy;
        options.to = "~/copies".to_string();

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "~/private",
                vec![make_file_action(".zshrc", ".zshrc", options)],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should resolve ~/ paths for copy operations");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_copy(
                home.path().join("private").join(".zshrc"),
                home.path().join("copies").join(".zshrc"),
            )])
        );
    }

    #[test]
    fn expand_multiple_sources_into_flat_concrete_actions() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");
        let mut config_options = default_options();
        config_options.to = "~/.config".to_string();
        let config_source = repository.join("config");

        let intent_plan = make_intent_plan(
            &repository,
            vec![
                make_intent_source(
                    "home-files",
                    vec![make_file_action(".zshrc", ".zshrc", default_options())],
                ),
                make_intent_source(
                    "config",
                    vec![make_select_action(
                        SelectionType::Dot,
                        vec![],
                        config_options,
                    )],
                ),
            ],
        );

        let dir_reader = FnDirectoryReader(|path| {
            assert_eq!(path, config_source.as_path());
            Ok(vec![".nvim".to_string(), ".tmux".to_string()])
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should combine actions from multiple sources");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.join("home-files").join(".zshrc"),
                    home.path().join(".zshrc"),
                ),
                expected_symlink(
                    repository.join("config").join(".nvim"),
                    home.path().join(".config").join(".nvim"),
                ),
                expected_symlink(
                    repository.join("config").join(".tmux"),
                    home.path().join(".config").join(".tmux"),
                ),
            ])
        );
    }

    #[test]
    fn derive_operation_plan_uses_the_provided_home_directory() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".zshrc"), "export TEST=1\n")
            .expect("source file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_file_action(".zshrc", ".zshrc", default_options())],
            )],
        );

        let operation_plan = derive_operation_plan_entrypoint(&intent_plan, home.path())
            .expect("operation planning should use the provided home directory");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".").join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }

    #[test]
    fn cannot_derive_operation_plan_when_a_source_directory_is_missing() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let missing_source = repository.path().join("missing");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "missing",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(|path| {
            let path = path.to_path_buf();
            crate::fs_adapter::list_directory_entries(&path)
        });
        let context = make_operation_context(home.path().to_path_buf(), &dir_reader, None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("missing source directories should be preserved as planning issues");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Issue(OperationPlanningIssue {
                    path: missing_source,
                    source_availability: SourceAvailability::Missing,
                })],
            }
        );
    }

    #[test]
    fn exclude_ds_store_entries_during_selector_expansion() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".DS_Store"), "binary\n")
            .expect("global dotfile should be written");
        fs::write(repository.path().join(".zshrc"), "export TEST=1\n")
            .expect("selectable dotfile should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let context = make_operation_context(
            home.path().to_path_buf(),
            &dir_reader,
            Some(vec![".DS_Store"]),
        );
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should honor global excludes");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }

    #[test]
    fn exclude_multiple_global_selection_entries_during_selector_expansion() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".DS_Store"), "binary\n")
            .expect("first global dotfile should be written");
        fs::write(repository.path().join(".git"), "index\n")
            .expect("second global dotfile should be written");
        fs::write(repository.path().join(".zshrc"), "export TEST=1\n")
            .expect("selectable dotfile should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let dir_reader = FnDirectoryReader(crate::fs_adapter::list_directory_entries);
        let context = make_operation_context(
            home.path().to_path_buf(),
            &dir_reader,
            Some(vec![".DS_Store", ".git"]),
        );
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should honor multiple global excludes");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".zshrc"),
                home.path().join(".zshrc"),
            )])
        );
    }
}
