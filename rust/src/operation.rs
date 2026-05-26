//! Operation planning stage.
//!
//! Converts an [`IntentPlan`] into a fully resolved [`OperationPlan`] by
//! expanding source-relative paths and selector patterns into concrete
//! filesystem operations. This stage only plans — no filesystem writes happen
//! here.
//!
//! The composition root in `lib.rs` wires a real source-availability boundary
//! into an [`OperationContext`]. Directory traversal stays in this module so
//! operation planning owns selection expansion semantics.

use crate::config::SelectionType;
use crate::{
    ActionMode, ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
    ProgramError, ResolvedOptions, SourceAvailability, SourceUnreadableKind, ValidatedGlobPattern,
    boundary::{
        SourceAvailabilityReader, SourcePathRequirement, SourceReadError, SourceReadErrorKind,
    },
    logging, path_resolution,
};
use log::Level;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecursiveDirectoryEntryKind {
    File,
    Directory,
}

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
    pub skip_reason: Option<OperationSkipReason>,
}

/// Reason an otherwise-planned concrete operation should be skipped at apply time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationSkipReason {
    MissingTargetDirectory,
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
    /// Boundary adapter for probing direct source path availability.
    pub(crate) source_reader: &'a dyn SourceAvailabilityReader,
    /// Platform- or configuration-derived exclusions applied to every selector.
    pub(crate) global_selection_excludes: Vec<ValidatedGlobPattern>,
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
        skip_reason: None,
        action,
    })
}

fn expand_select_action(
    source_root: &Path,
    select_action: &IntentSelectAction,
    home_directory: &Path,
    global_selection_excludes: &[ValidatedGlobPattern],
    source_reader: &dyn SourceAvailabilityReader,
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

    let mut excludes =
        Vec::with_capacity(global_selection_excludes.len() + select_action.exclude.len());
    excludes.extend_from_slice(global_selection_excludes);
    excludes.extend_from_slice(&select_action.exclude);

    let max_depth = if select_action.recursive {
        None
    } else {
        Some(0)
    };
    let entries = match list_directory_entries(source_root, max_depth) {
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
            should_include_selection_entry(entry, select_action.selection_type.clone(), &excludes)
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

            let source_availability = source_availability_from_result(
                source_reader
                    .ensure_source_path_readable(action.source(), SourcePathRequirement::File),
            );
            let skip_reason = if select_action.recursive
                && select_action.existing_directories_only
                && source_availability == SourceAvailability::Available
                && !target_directory_exists(action.target())
            {
                Some(OperationSkipReason::MissingTargetDirectory)
            } else {
                None
            };

            OperationEntry::Action(PlannedOperationAction {
                source_availability,
                skip_reason,
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

fn target_directory_exists(path: &Path) -> bool {
    path.parent().is_none_or(|parent| parent.is_dir())
}

fn list_directory_entries(
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
    let read_dir = read_directory(path)?;

    let mut entries = Vec::new();
    for entry in read_dir {
        let entry = read_dir_entry(path, entry)?;
        let file_type = read_entry_type(path, &entry)?;
        let kind = if file_type.is_dir() {
            RecursiveDirectoryEntryKind::Directory
        } else {
            RecursiveDirectoryEntryKind::File
        };

        entries.push((entry.file_name().to_string_lossy().into_owned(), kind));
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
    selection_type: SelectionType,
    excludes: &[ValidatedGlobPattern],
) -> bool {
    let entry_name = Path::new(entry)
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or(entry);
    let has_dot_prefix = entry_name.starts_with('.');
    let selected_by_type = match selection_type {
        SelectionType::Dot => has_dot_prefix,
        SelectionType::NonDot => !has_dot_prefix,
        SelectionType::All => true,
    };

    // Selection type decides the candidate set first; glob excludes further
    // reduce that set.
    selected_by_type
        && !excludes
            .iter()
            .any(|excluded| excluded.is_match(Path::new(entry)))
}

#[cfg(test)]
mod tests {
    use super::{
        FileOperation, OperationAction, OperationContext, OperationEntry, OperationPlan,
        OperationPlanningIssue, OperationSkipReason, PlannedOperationAction, SourceReadErrorKind,
        derive_operation_plan,
    };
    use crate::{
        ActionMode, ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
        IntentSource, ResolvedOptions, SelectionType, SourceAvailability, SourceUnreadableKind,
        ValidatedGlobPattern,
        boundary::{
            SourcePathRequirement, SourceReadError, test_doubles::FnSourceAvailabilityReader,
        },
        derive_operation_plan as derive_operation_plan_entrypoint,
    };
    use std::fs;
    use std::io::ErrorKind;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
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
        make_select_action_with_recursive(selection_type, false, exclude, options)
    }

    fn make_select_action_with_recursive(
        selection_type: SelectionType,
        recursive: bool,
        exclude: Vec<&str>,
        options: ResolvedOptions,
    ) -> IntentAction {
        IntentAction::Select(IntentSelectAction {
            selection_type,
            recursive,
            existing_directories_only: false,
            exclude: validated_patterns(exclude),
            options,
        })
    }

    fn make_select_action_with_recursive_directory_policy(
        selection_type: SelectionType,
        recursive: bool,
        existing_directories_only: bool,
        exclude: Vec<&str>,
        options: ResolvedOptions,
    ) -> IntentAction {
        IntentAction::Select(IntentSelectAction {
            selection_type,
            recursive,
            existing_directories_only,
            exclude: validated_patterns(exclude),
            options,
        })
    }

    fn validated_patterns(patterns: Vec<&str>) -> Vec<ValidatedGlobPattern> {
        patterns
            .into_iter()
            .map(|pattern| {
                ValidatedGlobPattern::parse_select_exclude(pattern)
                    .expect("test glob patterns should be valid")
            })
            .collect()
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
        global_selection_excludes: Option<Vec<&str>>,
    ) -> OperationContext<'a> {
        OperationContext {
            home_directory,
            source_reader: &READABLE_SOURCE_READER,
            global_selection_excludes: validated_patterns(
                global_selection_excludes.unwrap_or_default(),
            ),
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
                        skip_reason: None,
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
        let context = make_operation_context(home.path().to_path_buf(), None);
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
        let context = make_operation_context(home.path().to_path_buf(), None);
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

        let context = make_operation_context(home.path().to_path_buf(), None);
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

        let context = make_operation_context(home.path().to_path_buf(), None);
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
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::create_dir_all(repository.path().join("configs"))
            .expect("configs directory should be created");
        fs::write(
            repository.path().join("configs").join(".zshrc"),
            "export TEST=1\n",
        )
        .expect("dotfile should be written");
        fs::write(
            repository.path().join("configs").join("notes.txt"),
            "notes\n",
        )
        .expect("excluded file should be written");
        fs::write(
            repository.path().join("configs").join("tmux.conf"),
            "set -g mouse on\n",
        )
        .expect("selected non-dotfile should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::NonDot,
                    vec!["notes.txt"],
                    default_options(),
                )],
            )],
        );

        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand non-dotfile selection");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join("configs").join("tmux.conf"),
                home.path().join("tmux.conf"),
            )])
        );
    }

    #[test]
    fn expand_all_selection_entries_when_type_is_all() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::create_dir_all(repository.path().join("configs"))
            .expect("configs directory should be created");
        fs::write(
            repository.path().join("configs").join(".zshrc"),
            "export TEST=1\n",
        )
        .expect("dotfile should be written");
        fs::write(
            repository.path().join("configs").join("notes.txt"),
            "notes\n",
        )
        .expect("excluded file should be written");
        fs::write(
            repository.path().join("configs").join("tmux.conf"),
            "set -g mouse on\n",
        )
        .expect("selected file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::All,
                    vec!["notes.txt"],
                    default_options(),
                )],
            )],
        );

        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should expand all selection entries");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.path().join("configs").join(".zshrc"),
                    home.path().join(".zshrc")
                ),
                expected_symlink(
                    repository.path().join("configs").join("tmux.conf"),
                    home.path().join("tmux.conf"),
                ),
            ])
        );
    }

    #[test]
    fn apply_glob_excludes_after_dot_selection_type_filtering() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".keep"), "dot file\n")
            .expect("selected dotfile should be written");
        fs::write(repository.path().join(".ignore.tmp"), "dot temp file\n")
            .expect("excluded dotfile should be written");
        fs::write(repository.path().join("notes.tmp"), "plain temp file\n")
            .expect("non-dot file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec!["*.tmp"],
                    default_options(),
                )],
            )],
        );

        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should apply dot-selection and excludes");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".keep"),
                home.path().join(".keep"),
            )])
        );
    }

    #[test]
    fn apply_recursive_glob_excludes_to_source_relative_paths() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::create_dir_all(
            repository
                .path()
                .join("bundle")
                .join("nested")
                .join("private"),
        )
        .expect("nested source directories should be created");
        fs::write(repository.path().join("bundle").join("keep.conf"), "keep\n")
            .expect("top-level file should be written");
        fs::write(
            repository
                .path()
                .join("bundle")
                .join("nested")
                .join("cache.tmp"),
            "cache\n",
        )
        .expect("nested temporary file should be written");
        fs::write(
            repository
                .path()
                .join("bundle")
                .join("nested")
                .join("private")
                .join("secret.conf"),
            "secret\n",
        )
        .expect("nested excluded file should be written");
        fs::write(
            repository
                .path()
                .join("bundle")
                .join("nested")
                .join("public.conf"),
            "public\n",
        )
        .expect("nested selected file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "bundle",
                vec![make_select_action_with_recursive(
                    SelectionType::All,
                    true,
                    vec!["**/*.tmp", "nested/private/**"],
                    default_options(),
                )],
            )],
        );

        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should apply recursive glob excludes");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.path().join("bundle").join("keep.conf"),
                    home.path().join("keep.conf"),
                ),
                expected_symlink(
                    repository
                        .path()
                        .join("bundle")
                        .join("nested")
                        .join("public.conf"),
                    home.path().join("nested").join("public.conf"),
                ),
            ])
        );
    }

    #[test]
    fn keep_recursive_selection_flat_on_shallow_directories() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".zshrc"), "export TEST=1\n")
            .expect("dotfile should be written");
        fs::write(repository.path().join("notes.txt"), "plain file\n")
            .expect("file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action_with_recursive(
                    SelectionType::All,
                    true,
                    vec![],
                    default_options(),
                )],
            )],
        );
        let context = make_operation_context(home.path().to_path_buf(), None);

        let recursive_plan = derive_operation_plan(&intent_plan, &context)
            .expect("recursive selection should expand shallow entries");
        let direct_plan = derive_operation_plan(
            &make_intent_plan(
                repository.path(),
                vec![make_intent_source(
                    ".",
                    vec![make_select_action(
                        SelectionType::All,
                        vec![],
                        default_options(),
                    )],
                )],
            ),
            &context,
        )
        .expect("direct selection should expand the same shallow entries");

        assert_eq!(recursive_plan, direct_plan);
    }

    #[test]
    fn expand_recursive_selection_entries_in_depth_first_order() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join("z-last.txt"), "z\n")
            .expect("top-level file should be written");
        fs::write(repository.path().join("a-first.txt"), "a\n")
            .expect("top-level file should be written");
        fs::create_dir_all(repository.path().join("nested").join("inner"))
            .expect("nested directories should be created");
        fs::write(repository.path().join("nested").join("b-middle.txt"), "b\n")
            .expect("nested file should be written");
        fs::write(
            repository
                .path()
                .join("nested")
                .join("inner")
                .join("c-deep.txt"),
            "c\n",
        )
        .expect("deeply nested file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action_with_recursive(
                    SelectionType::All,
                    true,
                    vec![],
                    default_options(),
                )],
            )],
        );
        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("recursive selection should expand nested files");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.path().join("a-first.txt"),
                    home.path().join("a-first.txt"),
                ),
                expected_symlink(
                    repository.path().join("nested").join("b-middle.txt"),
                    home.path().join("nested").join("b-middle.txt"),
                ),
                expected_symlink(
                    repository
                        .path()
                        .join("nested")
                        .join("inner")
                        .join("c-deep.txt"),
                    home.path().join("nested").join("inner").join("c-deep.txt"),
                ),
                expected_symlink(
                    repository.path().join("z-last.txt"),
                    home.path().join("z-last.txt"),
                ),
            ])
        );
    }

    #[test]
    fn mark_recursive_entries_as_skipped_when_target_directories_are_missing() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::create_dir_all(repository.path().join("nested"))
            .expect("nested source directory should be created");
        fs::write(repository.path().join("nested").join("tool.conf"), "tool\n")
            .expect("nested source file should be written");
        fs::create_dir_all(home.path().join("profiles"))
            .expect("top-level target directory should be created");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action_with_recursive_directory_policy(
                    SelectionType::All,
                    true,
                    true,
                    vec![],
                    ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "profiles".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                )],
            )],
        );
        let context = make_operation_context(home.path().to_path_buf(), None);

        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("recursive selection should keep skipped entries in the plan");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Action(PlannedOperationAction {
                    action: expected_symlink(
                        repository.path().join("nested").join("tool.conf"),
                        home.path()
                            .join("profiles")
                            .join("nested")
                            .join("tool.conf"),
                    ),
                    source_availability: SourceAvailability::Available,
                    skip_reason: Some(OperationSkipReason::MissingTargetDirectory),
                })],
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn preserve_a_planning_issue_when_recursive_selection_cannot_read_a_nested_directory() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");

        fs::create_dir_all(repository.path().join("nested"))
            .expect("nested directory should be created");
        fs::write(
            repository.path().join("nested").join("visible.txt"),
            "visible\n",
        )
        .expect("visible file should be written");
        let unreadable_directory = repository.path().join("nested").join("private");
        fs::create_dir_all(&unreadable_directory).expect("private directory should be created");
        fs::write(unreadable_directory.join("hidden.txt"), "hidden\n")
            .expect("hidden file should be written");
        fs::set_permissions(&unreadable_directory, fs::Permissions::from_mode(0o0))
            .expect("private directory permissions should be updated");

        let permission_error_message = fs::read_dir(&unreadable_directory)
            .expect_err("unreadable directory should fail to read")
            .to_string();

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "nested",
                vec![make_select_action_with_recursive(
                    SelectionType::All,
                    true,
                    vec![],
                    default_options(),
                )],
            )],
        );
        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("recursive selection should preserve nested read failures");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Issue(OperationPlanningIssue {
                    path: unreadable_directory,
                    source_availability: SourceAvailability::Unreadable {
                        kind: SourceUnreadableKind::PermissionDenied,
                        message: permission_error_message,
                    },
                })],
            }
        );
    }

    #[test]
    fn map_recursive_source_read_errors_to_the_expected_availability_kinds() {
        let path = PathBuf::from("/repo-root/nested");

        let missing = super::source_read_error(
            &path,
            std::io::Error::new(ErrorKind::NotFound, "missing directory"),
        );
        let unexpected = super::source_read_error(&path, std::io::Error::other("boom"));

        assert_eq!(missing.path, path);
        assert_eq!(missing.kind, SourceReadErrorKind::Missing);
        assert_eq!(unexpected.path, path);
        assert_eq!(unexpected.kind, SourceReadErrorKind::UnexpectedIo);
    }

    #[test]
    fn preserve_directory_open_errors_as_source_read_errors() {
        let root = TempDir::new().expect("temporary root should be created");
        let missing_directory = root.path().join("missing");

        let error = super::read_directory(&missing_directory)
            .expect_err("missing directory reads should be reported as errors");

        assert_eq!(error.path, missing_directory);
        assert_eq!(error.kind, SourceReadErrorKind::Missing);
    }

    #[test]
    fn preserve_directory_iteration_errors_as_source_read_errors() {
        let directory = PathBuf::from("/repo-root/configs");

        let error = super::read_dir_entry(
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

        let error = super::map_entry_type_result(
            &directory,
            Err(std::io::Error::other("file type is unavailable")),
        )
        .expect_err("entry type failures should be preserved");

        assert_eq!(error.path, directory);
        assert_eq!(error.kind, SourceReadErrorKind::UnexpectedIo);
        assert_eq!(error.message, "file type is unavailable");
    }

    #[test]
    fn preserve_a_missing_directory_listing_as_a_planning_issue() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let missing_source = repository.path().join("configs");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("selector expansion should preserve planning issues");

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
    fn preserve_an_unexpected_directory_listing_error_as_a_planning_issue() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let non_directory_source = repository.path().join("configs");
        fs::write(&non_directory_source, "not a directory\n")
            .expect("non-directory source should be created");

        let unexpected_message = fs::read_dir(&non_directory_source)
            .expect_err("reading a regular file as a directory should fail")
            .to_string();

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    SelectionType::Dot,
                    vec![],
                    default_options(),
                )],
            )],
        );

        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("selector expansion should preserve planning issues");

        assert_eq!(
            operation_plan,
            OperationPlan {
                entries: vec![OperationEntry::Issue(OperationPlanningIssue {
                    path: non_directory_source,
                    source_availability: SourceAvailability::Unreadable {
                        kind: SourceUnreadableKind::UnexpectedIo,
                        message: unexpected_message,
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

        let source_reader = FnSourceAvailabilityReader(|path, _| {
            Err(SourceReadError {
                path: path.to_path_buf(),
                kind: SourceReadErrorKind::Missing,
                message: "missing".to_string(),
            })
        });
        let context = OperationContext {
            home_directory: home.path().to_path_buf(),
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
        let source_reader = FnSourceAvailabilityReader(|_, _| Ok(()));
        let available_context = OperationContext {
            home_directory: home.path().to_path_buf(),
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
        let source_reader = FnSourceAvailabilityReader(|path, _| {
            Err(SourceReadError {
                path: path.to_path_buf(),
                kind: SourceReadErrorKind::PermissionDenied,
                message: "denied".to_string(),
            })
        });
        let context = OperationContext {
            home_directory: home.path().to_path_buf(),
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
                    skip_reason: None,
                })],
            }
        );

        let readable_source_reader =
            FnSourceAvailabilityReader(|_, requirement| match requirement {
                SourcePathRequirement::Directory => Ok(()),
                SourcePathRequirement::File => Ok(()),
            });
        let readable_context = OperationContext {
            home_directory: home.path().to_path_buf(),
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
        let context = make_operation_context(home.path().to_path_buf(), None);
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
        let context = make_operation_context(home.path().to_path_buf(), None);
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
        let repository = TempDir::new().expect("temporary repository should be created");
        let mut config_options = default_options();
        config_options.to = "~/.config".to_string();
        fs::create_dir_all(repository.path().join("home-files"))
            .expect("home-files directory should be created");
        fs::write(
            repository.path().join("home-files").join(".zshrc"),
            "export TEST=1\n",
        )
        .expect("home source file should be written");
        fs::create_dir_all(repository.path().join("config"))
            .expect("config directory should be created");
        fs::write(
            repository.path().join("config").join(".nvim"),
            "set number\n",
        )
        .expect("first config file should be written");
        fs::write(
            repository.path().join("config").join(".tmux"),
            "set -g mouse on\n",
        )
        .expect("second config file should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
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
        let context = make_operation_context(home.path().to_path_buf(), None);
        let operation_plan = derive_operation_plan(&intent_plan, &context)
            .expect("operation planning should combine actions from multiple sources");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![
                expected_symlink(
                    repository.path().join("home-files").join(".zshrc"),
                    home.path().join(".zshrc"),
                ),
                expected_symlink(
                    repository.path().join("config").join(".nvim"),
                    home.path().join(".config").join(".nvim"),
                ),
                expected_symlink(
                    repository.path().join("config").join(".tmux"),
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

        let context = make_operation_context(home.path().to_path_buf(), None);
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
        let context = make_operation_context(home.path().to_path_buf(), Some(vec!["**/.DS_Store"]));
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
        let context = make_operation_context(
            home.path().to_path_buf(),
            Some(vec!["**/.DS_Store", "**/.git"]),
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
