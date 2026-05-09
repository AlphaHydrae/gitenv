use crate::{
    ActionMode, ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
    ProgramError, ResolvedOptions, fs_adapter, logging,
};
use log::Level;
use std::path::{Path, PathBuf};

/// Concrete, execution-shaped planning output.
///
/// Unlike `IntentPlan`, this stage resolves repository-relative source roots
/// and home-relative target directories into explicit filesystem operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationPlan {
    /// Ordered list of concrete operations to inspect or execute.
    pub actions: Vec<OperationAction>,
}

/// A concrete operation ready for status inspection or filesystem execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationAction {
    Copy(FileOperation),
    Symlink(FileOperation),
}

/// A concrete file operation with fully resolved source and target paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOperation {
    pub source: PathBuf,
    pub target: PathBuf,
    pub mkdir: bool,
    pub conflict_policy: ConflictPolicy,
}

/// Derives the operation plan from an intent plan using real filesystem reads.
pub fn derive_operation_plan(intent_plan: &IntentPlan) -> Result<OperationPlan, ProgramError> {
    derive_operation_plan_with_injectables(
        intent_plan,
        &current_home_directory,
        &fs_adapter::list_directory_entries,
    )
}

/// Like `derive_operation_plan` but accepts injected path-resolution context
/// and directory access for deterministic tests and composition-root injection.
///
/// Target directories are resolved relative to `home_directory` unless they are
/// absolute or already home-prefixed. Relative source roots are resolved
/// relative to the intent plan's repository root.
pub fn derive_operation_plan_with_injectables(
    intent_plan: &IntentPlan,
    get_home_directory: &impl Fn() -> Result<PathBuf, ProgramError>,
    list_directory: &impl Fn(&Path) -> Result<Vec<String>, ProgramError>,
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

    let home_directory = get_home_directory()?;
    let repository_root = resolve_repository_root(&intent_plan.repository, &home_directory);
    let mut actions = Vec::new();

    for source in &intent_plan.sources {
        let source_root = resolve_source_root(&repository_root, &source.from, &home_directory);

        for action in &source.actions {
            match action {
                IntentAction::File(file_action) => {
                    actions.push(expand_file_action(
                        &source_root,
                        file_action,
                        &home_directory,
                    ));
                }
                IntentAction::Select(select_action) => {
                    actions.extend(expand_select_action(
                        &source_root,
                        select_action,
                        &home_directory,
                        list_directory,
                    )?);
                }
            }
        }
    }

    logging::operation(
        Level::Info,
        "operation_plan_success",
        format!("actions={}", actions.len()),
    );

    Ok(OperationPlan { actions })
}

fn current_home_directory() -> Result<PathBuf, ProgramError> {
    logging::system(Level::Trace, "resolve_home_directory", "source=environment");
    fs_adapter::resolve_home_directory(&|name| std::env::var_os(name))
}

fn resolve_repository_root(repository: &str, home_directory: &Path) -> PathBuf {
    resolve_home_prefixed_or_literal_path(repository, home_directory)
}

fn resolve_source_root(
    repository_root: &Path,
    source_from: &str,
    home_directory: &Path,
) -> PathBuf {
    let path = Path::new(source_from);
    if path.is_absolute() || source_from == "~" || source_from.starts_with("~/") {
        resolve_home_prefixed_or_literal_path(source_from, home_directory)
    } else {
        repository_root.join(path)
    }
}

fn resolve_target_directory(target_directory: &str, home_directory: &Path) -> PathBuf {
    let path = Path::new(target_directory);
    if path.is_absolute() || target_directory == "~" || target_directory.starts_with("~/") {
        resolve_home_prefixed_or_literal_path(target_directory, home_directory)
    } else {
        home_directory.join(path)
    }
}

fn resolve_home_prefixed_or_literal_path(path: &str, home_directory: &Path) -> PathBuf {
    if path == "~" {
        home_directory.to_path_buf()
    } else if let Some(suffix) = path.strip_prefix("~/") {
        home_directory.join(suffix)
    } else {
        PathBuf::from(path)
    }
}

fn expand_file_action(
    source_root: &Path,
    file_action: &IntentFileAction,
    home_directory: &Path,
) -> OperationAction {
    operation_action(
        source_root,
        &file_action.file,
        &file_action.as_name,
        &file_action.options,
        home_directory,
    )
}

fn expand_select_action(
    source_root: &Path,
    select_action: &IntentSelectAction,
    home_directory: &Path,
    list_directory: &impl Fn(&Path) -> Result<Vec<String>, ProgramError>,
) -> Result<Vec<OperationAction>, ProgramError> {
    let entries = list_directory(source_root)?;
    let total_entries = entries.len();
    let selected_entries = entries
        .into_iter()
        .filter(|entry| should_include_selection_entry(entry, select_action))
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

    Ok(selected_entries
        .into_iter()
        .map(|entry| {
            operation_action(
                source_root,
                &entry,
                &entry,
                &select_action.options,
                home_directory,
            )
        })
        .collect())
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

fn should_include_selection_entry(entry: &str, select_action: &IntentSelectAction) -> bool {
    let has_dot_prefix = entry.starts_with('.');
    let selected_by_dotfiles = if select_action.dotfiles {
        has_dot_prefix
    } else {
        !has_dot_prefix
    };

    selected_by_dotfiles
        && !select_action
            .exclude
            .iter()
            .any(|excluded| excluded == entry)
}

#[cfg(test)]
mod tests {
    use super::{
        FileOperation, OperationAction, OperationPlan, derive_operation_plan,
        derive_operation_plan_with_injectables,
    };
    use crate::{
        ActionMode, ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
        IntentSource, ProgramError, ResolvedOptions,
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
        dotfiles: bool,
        exclude: Vec<&str>,
        options: ResolvedOptions,
    ) -> IntentAction {
        IntentAction::Select(IntentSelectAction {
            dotfiles,
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

    // ---------------------------------------------------------------------------
    // Operation plan constructors
    // ---------------------------------------------------------------------------

    fn expected_operation_plan(actions: Vec<OperationAction>) -> OperationPlan {
        OperationPlan { actions }
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

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    #[test]
    fn expand_selected_dotfiles_into_concrete_file_actions() {
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
                    true,
                    vec![".gitconfig"],
                    default_options(),
                )],
            )],
        );

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|path| {
                let path = path.to_path_buf();
                crate::fs_adapter::list_directory_entries(&path)
            },
        )
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

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &crate::fs_adapter::list_directory_entries,
        )
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
    fn expand_selected_dotfiles_into_sorted_actions() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        fs::write(repository.path().join(".z-last"), "a\n").expect("dotfile should be written");
        fs::write(repository.path().join(".a-first"), "b\n").expect("dotfile should be written");
        fs::write(repository.path().join(".m-middle"), "c\n").expect("dotfile should be written");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_select_action(true, vec![], default_options())],
            )],
        );

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|path| {
                let path = path.to_path_buf();
                crate::fs_adapter::list_directory_entries(&path)
            },
        )
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
                vec![make_select_action(true, vec![], default_options())],
            )],
        );

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|path| {
                let path = path.to_path_buf();
                crate::fs_adapter::list_directory_entries(&path)
            },
        )
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
    fn expand_non_dotfiles_when_dotfiles_selector_is_disabled() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_select_action(
                    false,
                    vec!["notes.txt"],
                    default_options(),
                )],
            )],
        );

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|_| {
                Ok(vec![
                    ".zshrc".to_string(),
                    "notes.txt".to_string(),
                    "tmux.conf".to_string(),
                ])
            },
        )
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
    fn propagate_directory_listing_errors_while_expanding_selectors() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = PathBuf::from("/repo-root");

        let intent_plan = make_intent_plan(
            &repository,
            vec![make_intent_source(
                "configs",
                vec![make_select_action(true, vec![], default_options())],
            )],
        );

        let error = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|path| {
                Err(ProgramError::ReadSourceDirectory {
                    path: path.to_path_buf(),
                    message: "boom".to_string(),
                })
            },
        )
        .expect_err("selector expansion should propagate listing failures");

        assert_eq!(
            error,
            ProgramError::ReadSourceDirectory {
                path: PathBuf::from("/repo-root/configs"),
                message: "boom".to_string(),
            }
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

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &crate::fs_adapter::list_directory_entries,
        )
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

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &crate::fs_adapter::list_directory_entries,
        )
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
                    vec![make_select_action(true, vec![], config_options)],
                ),
            ],
        );

        let operation_plan = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|path| {
                assert_eq!(path, config_source.as_path());
                Ok(vec![".nvim".to_string(), ".tmux".to_string()])
            },
        )
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
    fn derive_operation_plan_uses_the_real_home_directory() {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .expect("HOME should be available for the test environment");
        let repository = TempDir::new().expect("temporary repository should be created");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                ".",
                vec![make_file_action(".zshrc", ".zshrc", default_options())],
            )],
        );

        let operation_plan = derive_operation_plan(&intent_plan)
            .expect("operation planning should use the current HOME directory");

        assert_eq!(
            operation_plan,
            expected_operation_plan(vec![expected_symlink(
                repository.path().join(".").join(".zshrc"),
                home.join(".zshrc"),
            )])
        );
    }

    #[test]
    fn return_read_source_directory_error_for_missing_source_directory() {
        let home = TempDir::new().expect("temporary home directory should be created");
        let repository = TempDir::new().expect("temporary repository should be created");
        let missing_source = repository.path().join("missing");

        let intent_plan = make_intent_plan(
            repository.path(),
            vec![make_intent_source(
                "missing",
                vec![make_select_action(true, vec![], default_options())],
            )],
        );

        let error = derive_operation_plan_with_injectables(
            &intent_plan,
            &|| Ok(home.path().to_path_buf()),
            &|path| {
                let path = path.to_path_buf();
                crate::fs_adapter::list_directory_entries(&path)
            },
        )
        .expect_err("missing source directories should fail with typed error");

        assert!(matches!(
            &error,
            ProgramError::ReadSourceDirectory { path, message }
                if path == &missing_source && !message.is_empty()
        ));
    }
}
