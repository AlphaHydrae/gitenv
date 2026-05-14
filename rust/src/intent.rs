use crate::ProgramError;
use crate::boundary::{ConfigReader, DirectoryProbe, EnvironmentReader};
use crate::config::{
    ActionMode, Config, ConfigItem, Defaults, Guard, Include, LoadedConfig, SelectionType,
    SourceRoot,
};
use crate::logging;
use crate::path_resolution;
use log::Level;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Determines what the executor does when the destination file already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// Leave the destination untouched and skip the action.
    Skip,
    /// Overwrite the destination file without keeping a backup.
    Overwrite,
    /// Overwrite the destination file after saving a backup copy.
    OverwriteWithBackup,
}

/// Resolved execution options for a planned action, derived from the global
/// defaults merged with any source-level and item-level overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOptions {
    pub mode: ActionMode,
    pub to: String,
    pub mkdir: bool,
    pub conflict_policy: ConflictPolicy,
}

/// The intent-stage planning output derived from configuration semantics.
///
/// This stage resolves defaults, source roots, includes, and guards, but keeps
/// selector intent (for example selection scope and `exclude`) unexpanded.
/// Concrete path and selector expansion is the responsibility of the
/// operation stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentPlan {
    /// Canonical path to the repository root (from the top-level config).
    pub repository: String,
    /// Ordered list of sources with their resolved intent actions.
    pub sources: Vec<IntentSource>,
}

/// A resolved source directory together with the actions to be run from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentSource {
    /// Resolved filesystem path to the source directory.
    pub from: String,
    pub actions: Vec<IntentAction>,
}

/// A single resolved action to be executed against a source directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentAction {
    /// Copy or link a specific named file.
    File(IntentFileAction),
    /// Copy or link all matching files from the source directory.
    Select(IntentSelectAction),
}

/// Resolved action that operates on a single named file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentFileAction {
    /// Source-relative filename to operate on.
    pub file: String,
    /// Destination filename; defaults to `file` when no `as` override is set.
    pub as_name: String,
    pub options: ResolvedOptions,
}

/// Resolved action that operates on a glob-selected set of files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentSelectAction {
    /// Selection scope used by operation-stage selector expansion.
    pub selection_type: SelectionType,
    /// Enables recursive directory traversal during operation-stage expansion.
    pub recursive: bool,
    /// Filenames explicitly excluded from selection.
    pub exclude: Vec<String>,
    pub options: ResolvedOptions,
}

/// Stage-owned context for intent planning, carrying resolved runtime state
/// and trait-backed dependency references.
///
/// The composition root creates a `RealBoundary`-backed context; tests create
/// lightweight local doubles so no real filesystem or environment access is
/// needed.
pub(crate) struct IntentContext<'a> {
    /// Resolved home directory for the current process invocation.
    /// Carried as data rather than as a trait method so helper signatures
    /// stay small and the context fully represents stage runtime inputs.
    pub(crate) home_directory: PathBuf,
    /// Boundary adapter for environment variable reads.
    pub(crate) env_reader: &'a dyn EnvironmentReader,
    /// Boundary adapter for directory existence probes (used for guards).
    pub(crate) dir_probe: &'a dyn DirectoryProbe,
    /// Boundary adapter for config file reads (used for include resolution).
    pub(crate) config_reader: &'a dyn ConfigReader,
}

/// Internal planning function accepting a stage-owned context for
/// deterministic tests and composition-root injection.
///
/// `IntentContext.config_reader` is called once per include path. Tests supply
/// a local double that returns pre-parsed configs from an in-memory map.
pub(crate) fn derive_intent_plan(
    loaded_config: &LoadedConfig,
    context: &IntentContext,
) -> Result<IntentPlan, ProgramError> {
    let config: &Config = loaded_config;
    logging::intent(
        Level::Debug,
        "intent_plan_start",
        format!(
            "repository={} sources={} includes={}",
            config.repository,
            config.sources.len(),
            config.includes.len()
        ),
    );

    let mut missing_env: BTreeSet<String> = BTreeSet::new();
    let mut missing_files: BTreeSet<PathBuf> = BTreeSet::new();
    // in_flight tracks the ancestor chain for cycle detection; seed with the
    // root config path so a self-referencing include is caught.
    let mut in_flight: Vec<PathBuf> = vec![loaded_config.path.clone()];
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();

    let sources = plan_sources_recursively(
        config,
        loaded_config.path.as_path(),
        context,
        &mut in_flight,
        &mut seen,
        &mut missing_env,
        &mut missing_files,
    )?;

    if !missing_env.is_empty() {
        return Err(ProgramError::MissingEnvironment {
            vars: missing_env.into_iter().collect(),
        });
    }

    if !missing_files.is_empty() {
        return Err(ProgramError::IncludeNotFound {
            paths: missing_files.into_iter().collect(),
        });
    }

    let mut action_count = 0;
    for source in &sources {
        action_count += source.actions.len();
    }
    logging::intent(
        Level::Info,
        "intent_plan_success",
        format!("sources={} actions={}", sources.len(), action_count),
    );

    Ok(IntentPlan {
        repository: config.repository.clone(),
        sources,
    })
}

/// Recursively plans sources for one config and all its transitive includes.
///
/// Returns the config's own planned sources followed by each include's planned
/// sources in declaration order (includes-after ordering: the including file's
/// sources always appear before the included ones).
///
/// Errors:
/// - `IncludeCycle`: returned immediately when a path already in the ancestor
///   chain is encountered again. The returned `cycle` field contains the full
///   repeated chain. This is a structural error that cannot be recovered from.
/// - `MissingEnvironment` and `IncludeNotFound` diagnostics are collected into
///   `missing_env` and `missing_files` and handled by the caller after the
///   full tree has been walked.
fn plan_sources_recursively(
    config: &Config,
    current_config_path: &Path,
    context: &IntentContext,
    in_flight: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    missing_env: &mut BTreeSet<String>,
    missing_files: &mut BTreeSet<PathBuf>,
) -> Result<Vec<IntentSource>, ProgramError> {
    // Plan this config's own sources first (includes-after ordering).
    let mut own_sources = Vec::new();
    for source in &config.sources {
        let from = match &source.from {
            SourceRoot::Path(path) => Some(path.clone()),
            SourceRoot::ExplicitPath { path } => Some(path.clone()),
            SourceRoot::Environment {
                env,
                optional: false,
            } => match context.env_reader.get_env_var(env) {
                Some(value) => Some(value),
                None => {
                    missing_env.insert(env.clone());
                    None
                }
            },
            SourceRoot::Environment {
                env,
                optional: true,
            } => context.env_reader.get_env_var(env),
        };

        let Some(from) = from else {
            continue;
        };

        // Source-level `to` overrides the global default for all items in this source.
        let source_to = source
            .to
            .as_deref()
            .unwrap_or(&config.defaults.to)
            .to_string();

        // Evaluate the guard if present. A false guard skips the entire source.
        // `source_to` and `directory_exists` paths may start with `~`; expand
        // them using HOME before probing the filesystem.
        if let Some(guard) = &source.guard {
            let satisfied = match guard {
                Guard::ToExists => {
                    let expanded = path_resolution::expand_home_prefixed_or_literal_path(
                        &source_to,
                        &context.home_directory,
                    );
                    context.dir_probe.is_directory(&expanded.to_string_lossy())
                }
                Guard::DirectoryExists(path) => {
                    let expanded = path_resolution::expand_home_prefixed_or_literal_path(
                        path,
                        &context.home_directory,
                    );
                    context.dir_probe.is_directory(&expanded.to_string_lossy())
                }
            };
            if !satisfied {
                continue;
            }
        }

        let actions = source
            .configs
            .iter()
            .map(|item| match item {
                ConfigItem::File(file_config) => {
                    let options = resolve_item_options(
                        &config.defaults,
                        &source_to,
                        file_config.mode.as_ref(),
                        file_config.to.as_deref(),
                        file_config.mkdir,
                        file_config.overwrite,
                        file_config.backup_on_overwrite,
                    )?;
                    Ok(IntentAction::File(IntentFileAction {
                        file: file_config.file.clone(),
                        as_name: file_config
                            .as_name
                            .clone()
                            .unwrap_or_else(|| file_config.file.clone()),
                        options,
                    }))
                }
                ConfigItem::Select(select_config) => {
                    let options = resolve_item_options(
                        &config.defaults,
                        &source_to,
                        select_config.mode.as_ref(),
                        select_config.to.as_deref(),
                        select_config.mkdir,
                        select_config.overwrite,
                        select_config.backup_on_overwrite,
                    )?;
                    Ok(IntentAction::Select(IntentSelectAction {
                        selection_type: select_config.selection_type.clone(),
                        recursive: select_config.recursive,
                        exclude: select_config.exclude.clone(),
                        options,
                    }))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        own_sources.push(IntentSource { from, actions });
    }

    // Resolve and plan each include, appending their sources after own sources.
    let mut included_sources = Vec::new();
    for include in &config.includes {
        let (include_path, optional) = match include {
            Include::Path { path, optional } => {
                let resolved =
                    resolve_include_path(path, current_config_path, &context.home_directory);
                (Some(resolved), *optional)
            }
            Include::Environment { env, optional } => match context.env_reader.get_env_var(env) {
                Some(value) => (Some(PathBuf::from(value)), *optional),
                None if *optional => (None, true),
                None => {
                    missing_env.insert(env.clone());
                    (None, false)
                }
            },
        };

        let Some(include_path) = include_path else {
            continue;
        };

        // Skip already-processed includes (dedup across the whole include tree).
        if seen.contains(&include_path) {
            continue;
        }

        // A path already in the ancestor chain means a cycle.
        if let Some(start) = in_flight.iter().position(|path| path == &include_path) {
            let mut cycle = in_flight[start..].to_vec();
            cycle.push(include_path.clone());
            return Err(ProgramError::IncludeCycle { cycle });
        }

        // Attempt to load the included config file.
        let loaded_included_config = match context.config_reader.read_config_file(&include_path) {
            Ok(c) => c,
            // Optional includes are silently skipped when the file is missing.
            Err(ProgramError::ConfigurationReadFailed { .. }) if optional => {
                seen.insert(include_path);
                continue;
            }
            // Required includes that cannot be read are collected for the
            // caller to surface as a single IncludeNotFound error.
            Err(ProgramError::ConfigurationReadFailed { .. }) => {
                missing_files.insert(include_path);
                continue;
            }
            // Parse errors and other structural failures are propagated
            // immediately - they indicate a malformed config, not a
            // missing file.
            Err(e) => return Err(e),
        };

        // Push to in_flight before recursing to make this path visible to
        // cycle detection in the subtree. Seen is updated only after full
        // processing completes so that in-flight ancestors are never mistaken
        // for already-finished (deduplicated) nodes.
        in_flight.push(include_path.clone());

        let sub_sources = plan_sources_recursively(
            &loaded_included_config,
            loaded_included_config.path.as_path(),
            context,
            in_flight,
            seen,
            missing_env,
            missing_files,
        )?;

        in_flight.pop();
        // Mark as seen after full processing so sibling branches that
        // reference the same path are deduplicated without re-processing.
        seen.insert(include_path);

        included_sources.extend(sub_sources);
    }

    own_sources.extend(included_sources);
    Ok(own_sources)
}

/// Resolves an include path into an absolute `PathBuf`.
///
/// - Absolute paths are returned unchanged.
/// - Paths starting with `~` are expanded using `home_directory` before
///   resolution.
/// - Relative paths are resolved relative to the declaring config file's
///   directory.
fn resolve_include_path(path: &str, current_config_path: &Path, home_directory: &Path) -> PathBuf {
    let include_path = path_resolution::expand_home_prefixed_or_literal_path(path, home_directory);
    if include_path.is_absolute() {
        return include_path;
    }
    current_config_path
        .parent()
        .map(|parent| parent.join(&include_path))
        .unwrap_or(include_path)
}

/// Maps the resolved `overwrite` and `backup_on_overwrite` boolean flags to the
/// corresponding `ConflictPolicy` variant.
fn resolve_conflict_policy(overwrite: bool, backup_on_overwrite: bool) -> ConflictPolicy {
    if !overwrite {
        ConflictPolicy::Skip
    } else if backup_on_overwrite {
        ConflictPolicy::OverwriteWithBackup
    } else {
        ConflictPolicy::Overwrite
    }
}

/// Resolves execution options for a single config item by merging item-level
/// overrides on top of the inherited defaults and source-level destination.
///
/// Returns `InvalidConfiguration` immediately when the item explicitly sets
/// both `overwrite: false` and `backup_on_overwrite: true`. This combination
/// is meaningless (backup is irrelevant when overwrite is disabled) and most
/// likely indicates a config mistake.
///
/// Inherited defaults are never rejected: an item that falls back to the
/// global `overwrite: false, backup_on_overwrite: true` default resolves to
/// `ConflictPolicy::Skip` without error.
fn resolve_item_options(
    defaults: &Defaults,
    source_to: &str,
    mode: Option<&ActionMode>,
    to: Option<&str>,
    mkdir: Option<bool>,
    overwrite: Option<bool>,
    backup_on_overwrite: Option<bool>,
) -> Result<ResolvedOptions, ProgramError> {
    if overwrite == Some(false) && backup_on_overwrite == Some(true) {
        return Err(ProgramError::InvalidConfiguration {
            message: "item-level `overwrite: false` with `backup_on_overwrite: true` is not a valid combination"
                .to_string(),
        });
    }
    Ok(ResolvedOptions {
        mode: mode.unwrap_or(&defaults.mode).clone(),
        to: to.unwrap_or(source_to).to_string(),
        mkdir: mkdir.unwrap_or(defaults.mkdir),
        conflict_policy: resolve_conflict_policy(
            overwrite.unwrap_or(defaults.overwrite),
            backup_on_overwrite.unwrap_or(defaults.backup_on_overwrite),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ConflictPolicy, IntentAction, IntentContext, IntentFileAction, IntentPlan,
        IntentSelectAction, IntentSource, ResolvedOptions, derive_intent_plan,
    };
    use crate::boundary::{ConfigReader, DirectoryProbe, EnvironmentReader};
    use crate::{
        ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
        ProgramError, SelectConfig, SelectionType, Source, SourceRoot,
        derive_intent_plan as derive_intent_plan_entrypoint,
    };
    use std::path::{Path, PathBuf};

    // ---------------------------------------------------------------------------
    // Test double types — lightweight trait implementations for unit tests.
    //
    // These replace the old free-function helpers (`missing_environment`,
    // `directory_missing`, `unexpected_include_read`) with trait-backed doubles
    // that fit the new `IntentContext` API.
    // ---------------------------------------------------------------------------

    /// Test double: returns `None` for every environment variable lookup.
    struct NoEnvVars;
    impl EnvironmentReader for NoEnvVars {
        fn get_env_var(&self, _: &str) -> Option<String> {
            None
        }
    }

    /// Test double: reports every path as absent (not a directory).
    struct NoDirectories;
    impl DirectoryProbe for NoDirectories {
        fn is_directory(&self, _: &str) -> bool {
            false
        }
    }

    /// Test double: errors immediately — used for tests that must not read any
    /// include files. Signals test mistakes if an include is unexpectedly read.
    struct RejectConfigRead;
    impl ConfigReader for RejectConfigRead {
        fn read_config_file(&self, _: &Path) -> Result<LoadedConfig, ProgramError> {
            Err(ProgramError::InvalidConfiguration {
                message: "includes not tested here".to_string(),
            })
        }
    }

    /// Test double: wraps a closure for environment variable reads, allowing
    /// per-test custom env resolution without requiring a separate struct.
    struct FnEnvReader<F: Fn(&str) -> Option<String>>(F);
    impl<F: Fn(&str) -> Option<String>> EnvironmentReader for FnEnvReader<F> {
        fn get_env_var(&self, name: &str) -> Option<String> {
            self.0(name)
        }
    }

    /// Test double: wraps a closure for directory existence probes.
    struct FnDirProbe<F: Fn(&str) -> bool>(F);
    impl<F: Fn(&str) -> bool> DirectoryProbe for FnDirProbe<F> {
        fn is_directory(&self, path: &str) -> bool {
            self.0(path)
        }
    }

    /// Test double: wraps a closure for config file reads.
    struct FnConfigReader<F: Fn(&Path) -> Result<LoadedConfig, ProgramError>>(F);
    impl<F: Fn(&Path) -> Result<LoadedConfig, ProgramError>> ConfigReader for FnConfigReader<F> {
        fn read_config_file(&self, path: &Path) -> Result<LoadedConfig, ProgramError> {
            self.0(path)
        }
    }

    /// Constructs an `IntentContext` from three boundary doubles using the
    /// standard test home directory.
    fn make_context<'a>(
        env_reader: &'a dyn EnvironmentReader,
        dir_probe: &'a dyn DirectoryProbe,
        config_reader: &'a dyn ConfigReader,
    ) -> IntentContext<'a> {
        IntentContext {
            home_directory: home_directory_for_test().to_path_buf(),
            env_reader,
            dir_probe,
            config_reader,
        }
    }

    #[test]
    fn reject_config_reader_reports_invalid_configuration() {
        let error = RejectConfigRead
            .read_config_file(Path::new("/tmp/unused.yml"))
            .expect_err("reject config reader should always fail");

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "includes not tested here".to_string(),
            }
        );
    }

    #[test]
    fn fn_config_reader_forwards_to_the_wrapped_closure() {
        let reader = FnConfigReader(|path: &Path| crate::load_config(path));

        assert!(
            reader
                .read_config_file(Path::new(
                    "/tmp/definitely-missing-intent-config-reader-test.yml",
                ))
                .is_err()
        );
    }

    fn home_directory_for_test() -> &'static Path {
        Path::new("/home/tester")
    }

    // ---------------------------------------------------------------------------
    // Helper constructors
    // ---------------------------------------------------------------------------

    fn make_config(sources: Vec<Source>) -> Config {
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources,
            includes: vec![],
        }
    }

    fn make_source(from: SourceRoot, configs: Vec<ConfigItem>) -> Source {
        Source {
            from,
            to: None,
            guard: None,
            configs,
        }
    }

    fn make_file_config_item(file: &str) -> ConfigItem {
        ConfigItem::File(FileConfig {
            file: file.to_string(),
            as_name: None,
            mode: None,
            to: None,
            mkdir: None,
            overwrite: None,
            backup_on_overwrite: None,
        })
    }

    /// Minimal planned file action with symlink/skip defaults pointing to `~`.
    fn expected_file_action(file: &str) -> IntentAction {
        IntentAction::File(IntentFileAction {
            file: file.to_string(),
            as_name: file.to_string(),
            options: ResolvedOptions {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            },
        })
    }

    /// Minimal execution plan rooted at the standard test repository path.
    fn expected_plan(sources: Vec<IntentSource>) -> IntentPlan {
        IntentPlan {
            repository: "~/projects/env".to_string(),
            sources,
        }
    }

    fn loaded_config_for_test(config: Config, path: &Path) -> LoadedConfig {
        LoadedConfig {
            path: path.to_path_buf(),
            config,
        }
    }

    fn root_loaded_config(config: &Config) -> LoadedConfig {
        loaded_config_for_test(config.clone(), Path::new("/intent-tests/root.yml"))
    }

    fn loaded_config_for_path(config: &Config, path: Option<&Path>) -> LoadedConfig {
        loaded_config_for_test(
            config.clone(),
            path.unwrap_or_else(|| Path::new("/intent-tests/root.yml")),
        )
    }

    // ---------------------------------------------------------------------------
    // Intent plan derivation
    // ---------------------------------------------------------------------------

    #[test]
    fn derive_a_minimal_intent_plan_with_the_intent_entrypoint() {
        let config = make_config(vec![make_source(
            SourceRoot::Path(".".to_string()),
            vec![make_file_config_item(".zshrc")],
        )]);
        let plan =
            derive_intent_plan_entrypoint(&root_loaded_config(&config), home_directory_for_test())
                .expect("config should produce an intent plan");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![expected_file_action(".zshrc")],
            }])
        );
    }

    #[test]
    fn derive_a_deterministic_intent_plan_from_a_config() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Copy,
                to: "~/dest".to_string(),
                mkdir: false,
                overwrite: true,
                backup_on_overwrite: false,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![
                    ConfigItem::File(FileConfig {
                        file: ".zshrc".to_string(),
                        as_name: None,
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                    ConfigItem::File(FileConfig {
                        file: ".tmux".to_string(),
                        as_name: Some(".tmux.conf".to_string()),
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                    ConfigItem::Select(SelectConfig {
                        selection_type: SelectionType::Dot,
                        recursive: false,
                        exclude: vec![".git".to_string()],
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                ],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("config should produce an intent plan");

        let expected = IntentPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![
                    IntentAction::File(IntentFileAction {
                        file: ".zshrc".to_string(),
                        as_name: ".zshrc".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/dest".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    }),
                    IntentAction::File(IntentFileAction {
                        file: ".tmux".to_string(),
                        as_name: ".tmux.conf".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/dest".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    }),
                    IntentAction::Select(IntentSelectAction {
                        selection_type: SelectionType::Dot,
                        recursive: false,
                        exclude: vec![".git".to_string()],
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/dest".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    }),
                ],
            }],
        };

        assert_eq!(plan, expected);
    }

    #[test]
    fn derive_intent_plan_preserves_non_dot_selection_type() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Copy,
                to: "~/dest".to_string(),
                mkdir: false,
                overwrite: true,
                backup_on_overwrite: false,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    selection_type: SelectionType::NonDot,
                    recursive: false,
                    exclude: vec!["Makefile".to_string()],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("config should produce an intent plan");

        let expected = IntentPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::Select(IntentSelectAction {
                    selection_type: SelectionType::NonDot,
                    recursive: false,
                    exclude: vec!["Makefile".to_string()],
                    options: ResolvedOptions {
                        mode: ActionMode::Copy,
                        to: "~/dest".to_string(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Overwrite,
                    },
                })],
            }],
        };

        assert_eq!(plan, expected);
    }

    #[test]
    fn derive_intent_plan_preserves_all_selection_type() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: false,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("config".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    selection_type: SelectionType::All,
                    recursive: false,
                    exclude: vec![".backup".to_string(), ".tmp".to_string()],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("config should produce an intent plan");

        let expected = IntentPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![IntentSource {
                from: "config".to_string(),
                actions: vec![IntentAction::Select(IntentSelectAction {
                    selection_type: SelectionType::All,
                    recursive: false,
                    exclude: vec![".backup".to_string(), ".tmp".to_string()],
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }],
        };

        assert_eq!(plan, expected);
    }

    #[test]
    fn propagate_true_recursive_select_setting_to_intent_actions() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    selection_type: SelectionType::Dot,
                    recursive: true,
                    exclude: vec![".git".to_string()],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should propagate recursive select settings");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::Select(IntentSelectAction {
                    selection_type: SelectionType::Dot,
                    recursive: true,
                    exclude: vec![".git".to_string()],
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn derive_intent_plan_uses_real_environment_and_filesystem() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: false,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Environment {
                    env: "HOME".to_string(),
                    optional: false,
                },
                to: None,
                guard: Some(Guard::DirectoryExists(".".to_string())),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let expected_home = std::env::var_os("HOME")
            .expect("HOME should be set for process-backed environment lookup")
            .to_string_lossy()
            .into_owned();

        let plan =
            derive_intent_plan_entrypoint(&root_loaded_config(&config), home_directory_for_test())
                .expect("planning should succeed when the current directory exists");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: expected_home,
                actions: vec![expected_file_action(".zshrc")],
            }])
        );
    }

    #[test]
    fn resolve_environment_backed_sources_during_planning() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Copy,
                to: "~/.config".to_string(),
                mkdir: false,
                overwrite: true,
                backup_on_overwrite: false,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Environment {
                    env: "PRIVATE_ENV_DIR".to_string(),
                    optional: true,
                },
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".secrets".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };
        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &FnEnvReader(|name: &str| {
                    assert_eq!(name, "PRIVATE_ENV_DIR");
                    Some("~/projects/private-env".to_string())
                }),
                &NoDirectories,
                &RejectConfigRead,
            ),
        )
        .expect("planning should resolve the environment-backed source");

        let expected = IntentPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![IntentSource {
                from: "~/projects/private-env".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".secrets".to_string(),
                    as_name: ".secrets".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Copy,
                        to: "~/.config".to_string(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Overwrite,
                    },
                })],
            }],
        };

        assert_eq!(plan, expected);
    }

    #[test]
    fn plan_explicit_path_source_roots_as_literal_paths() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::ExplicitPath {
                    path: "$PRIVATE_ENV_DIR".to_string(),
                },
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should keep explicit path source roots as-is");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "$PRIVATE_ENV_DIR".to_string(),
                actions: vec![expected_file_action(".zshrc")],
            }])
        );
    }

    #[test]
    fn collapse_backup_preference_when_overwrite_is_disabled_in_planning() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should collapse overwrite-disabled defaults to skip");

        let expected = IntentPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".zshrc".to_string(),
                    as_name: ".zshrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }],
        };

        assert_eq!(plan, expected);
    }

    #[test]
    fn reject_missing_environment_variables_before_planning() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![
                Source {
                    from: SourceRoot::Environment {
                        env: "PRIVATE_ENV_DIR".to_string(),
                        optional: false,
                    },
                    to: None,
                    guard: None,
                    configs: vec![ConfigItem::File(FileConfig {
                        file: ".zshrc".to_string(),
                        as_name: None,
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    })],
                },
                Source {
                    from: SourceRoot::Environment {
                        env: "SSH_KEY_DIR".to_string(),
                        optional: false,
                    },
                    to: None,
                    guard: None,
                    configs: vec![ConfigItem::File(FileConfig {
                        file: ".ssh_config".to_string(),
                        as_name: None,
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    })],
                },
            ],
        };

        let error = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect_err("planning should fail when environment variables are missing");

        assert_eq!(
            error,
            ProgramError::MissingEnvironment {
                vars: vec!["PRIVATE_ENV_DIR".to_string(), "SSH_KEY_DIR".to_string()],
            }
        );
    }

    #[test]
    fn skip_optional_environment_backed_sources_when_the_variable_is_unset() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Environment {
                    env: "PRIVATE_ENV_DIR".to_string(),
                    optional: true,
                },
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should succeed when an optional environment-backed source is unset");

        assert_eq!(plan, expected_plan(vec![]));
    }

    // ---------------------------------------------------------------------------
    // Guards
    // ---------------------------------------------------------------------------

    #[test]
    fn source_level_to_overrides_default_to_in_planned_actions() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Environment {
                    env: "SOURCE_DIR".to_string(),
                    optional: false,
                },
                to: Some("~/Library/Application Support/Code/User".to_string()),
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &FnEnvReader(|name: &str| {
                    assert_eq!(name, "SOURCE_DIR");
                    Some("vscode".to_string())
                }),
                &NoDirectories,
                &RejectConfigRead,
            ),
        )
        .expect("planning should succeed with a source-level to");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "vscode".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: "settings.json".to_string(),
                    as_name: "settings.json".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~/Library/Application Support/Code/User".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn include_a_source_when_to_exists_guard_is_satisfied() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("vscode".to_string()),
                to: Some("~/Library/Application Support/Code/User".to_string()),
                guard: Some(Guard::ToExists),
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };
        // Simulate the destination directory being present after expansion via
        // the injected home directory.
        let known_dirs = std::collections::BTreeSet::from([
            "/home/tester/Library/Application Support/Code/User".to_string(),
        ]);

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &NoEnvVars,
                &FnDirProbe(|path| known_dirs.contains(path)),
                &RejectConfigRead,
            ),
        )
        .expect("planning should succeed when the to_exists guard is satisfied");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "vscode".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: "settings.json".to_string(),
                    as_name: "settings.json".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~/Library/Application Support/Code/User".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn exclude_a_source_when_to_exists_guard_is_not_satisfied() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("vscode".to_string()),
                to: Some("~/Library/Application Support/Code/User".to_string()),
                guard: Some(Guard::ToExists),
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };
        // Destination directory is absent after expansion via injected home.
        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should succeed when the to_exists guard is not satisfied");

        assert_eq!(plan, expected_plan(vec![]));
    }

    #[test]
    fn include_a_source_when_directory_exists_guard_is_satisfied() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("macos".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists("/Applications".to_string())),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".macos-defaults".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };
        let known_dirs = std::collections::BTreeSet::from(["/Applications".to_string()]);

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &NoEnvVars,
                &FnDirProbe(|path| known_dirs.contains(path)),
                &RejectConfigRead,
            ),
        )
        .expect("planning should succeed when the directory_exists guard is satisfied");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "macos".to_string(),
                actions: vec![expected_file_action(".macos-defaults")],
            }])
        );
    }

    #[test]
    fn exclude_a_source_when_directory_exists_guard_is_not_satisfied() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("macos".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists("/Applications".to_string())),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".macos-defaults".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should succeed when the directory_exists guard is not satisfied");

        assert_eq!(plan, expected_plan(vec![]));
    }

    #[test]
    fn exclude_a_source_when_real_directory_guard_points_to_a_regular_file() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("Cargo.toml")
                        .display()
                        .to_string(),
                )),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should skip sources when a real directory guard points to a file");

        assert_eq!(plan, expected_plan(vec![]));
    }

    #[test]
    fn exclude_a_source_when_real_directory_guard_points_to_a_missing_path() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists(
                    "/definitely-missing-gitenv-intent-probe".to_string(),
                )),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should skip sources when a real directory guard path is missing");

        assert_eq!(plan, expected_plan(vec![]));
    }

    // ---------------------------------------------------------------------------
    // Conflict policy resolution
    // ---------------------------------------------------------------------------

    #[test]
    fn include_a_source_when_to_exists_guard_uses_a_home_relative_destination() {
        // Verifies that a `~`-prefixed `to` path is expanded with the injected
        // home directory before the `to_exists` guard probes the filesystem.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("vscode".to_string()),
                to: Some("~/AppData/Roaming/Code/User".to_string()),
                guard: Some(Guard::ToExists),
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };
        // Guard receives the expanded absolute path after ~ expansion.
        let known_dirs =
            std::collections::BTreeSet::from(
                ["/home/tester/AppData/Roaming/Code/User".to_string()],
            );

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &NoEnvVars,
                &FnDirProbe(|path| known_dirs.contains(path)),
                &RejectConfigRead,
            ),
        )
        .expect("planning should succeed when to_exists guard uses a home-relative destination");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "vscode".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: "settings.json".to_string(),
                    as_name: "settings.json".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~/AppData/Roaming/Code/User".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn include_a_source_when_directory_exists_guard_uses_a_home_relative_path() {
        // Verifies that a `~`-prefixed `directory_exists` path is expanded
        // with the injected home directory before the guard probes the filesystem.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("work".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists("~/work-active".to_string())),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".workrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };
        let known_dirs = std::collections::BTreeSet::from(["/home/tester/work-active".to_string()]);

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &NoEnvVars,
                &FnDirProbe(|path| known_dirs.contains(path)),
                &RejectConfigRead,
            ),
        )
        .expect("planning should succeed when directory_exists guard uses a home-relative path");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "work".to_string(),
                actions: vec![expected_file_action(".workrc")],
            }])
        );
    }

    #[test]
    fn overwrite_with_backup_produces_backup_conflict_policy() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: true,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should produce overwrite-with-backup conflict policy");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".zshrc".to_string(),
                    as_name: ".zshrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::OverwriteWithBackup,
                    },
                })],
            }])
        );
    }

    // ---------------------------------------------------------------------------
    // Include planning
    //
    // These tests use `derive_intent_plan_with_injectables` and supply a
    // `read_config_file` closure that returns pre-built in-memory configs keyed
    // by path string, so no real files need to be written.
    // ---------------------------------------------------------------------------

    #[test]
    fn include_declared_files_sources_after_own_sources() {
        // Root declares .zshrc; includes /inc/a.yml which declares .tmux.conf.
        // Expected order: root's source first, then the include's source.
        let included = make_config(vec![make_source(
            SourceRoot::Path("inc".to_string()),
            vec![make_file_config_item(".tmux.conf")],
        )]);
        let root = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    assert_eq!(path, Path::new("/inc/a.yml"));
                    Ok(loaded_config_for_test(included.clone(), path))
                }),
            ),
        )
        .expect("planning should succeed with a valid include");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: ".".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "inc".to_string(),
                    actions: vec![expected_file_action(".tmux.conf")],
                },
            ])
        );
    }

    #[test]
    fn resolve_relative_include_paths_from_the_declaring_config_file() {
        let included = make_config(vec![make_source(
            SourceRoot::Path("inc".to_string()),
            vec![make_file_config_item(".tmux.conf")],
        )]);
        let root_path = Path::new("/configs/root.yml");
        let root = Config {
            includes: vec![Include::Path {
                path: "inc/a.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, Some(root_path)),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    assert_eq!(path, Path::new("/configs/inc/a.yml"));
                    Ok(loaded_config_for_test(included.clone(), path))
                }),
            ),
        )
        .expect("planning should resolve a relative include from the declaring file path");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: "root_src".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "inc".to_string(),
                    actions: vec![expected_file_action(".tmux.conf")],
                },
            ])
        );
    }

    #[test]
    fn resolve_nested_relative_include_paths_from_each_declaring_file() {
        let root_path = Path::new("/configs/root.yml");
        let c = make_config(vec![make_source(
            SourceRoot::Path("c_src".to_string()),
            vec![make_file_config_item(".bashrc")],
        )]);
        let a = Config {
            includes: vec![Include::Path {
                path: "nested/c.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("a_src".to_string()),
                vec![make_file_config_item(".aliases")],
            )])
        };
        let root = Config {
            includes: vec![Include::Path {
                path: "inc/a.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, Some(root_path)),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    if path == Path::new("/configs/inc/a.yml") {
                        Ok(loaded_config_for_test(a.clone(), path))
                    } else {
                        assert_eq!(path, Path::new("/configs/inc/nested/c.yml"));
                        Ok(loaded_config_for_test(c.clone(), path))
                    }
                }),
            ),
        )
        .expect("planning should resolve each nested relative include from its declaring file");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: "root_src".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "a_src".to_string(),
                    actions: vec![expected_file_action(".aliases")],
                },
                IntentSource {
                    from: "c_src".to_string(),
                    actions: vec![expected_file_action(".bashrc")],
                },
            ])
        );
    }

    #[test]
    fn resolve_home_relative_include_path_using_the_injected_home_directory() {
        // Verifies that a `~/...` include path is expanded with the injected
        // home directory before
        // the planner attempts to load the included config file.
        let included = make_config(vec![make_source(
            SourceRoot::Path("shared".to_string()),
            vec![make_file_config_item(".tmux.conf")],
        )]);
        let root = Config {
            includes: vec![Include::Path {
                path: "~/configs/shared.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    // The planner must resolve the ~ path to the absolute path.
                    assert_eq!(path, Path::new("/home/tester/configs/shared.yml"));
                    Ok(loaded_config_for_test(included.clone(), path))
                }),
            ),
        )
        .expect("planning should succeed when the include path uses a home-relative prefix");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: "root_src".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "shared".to_string(),
                    actions: vec![expected_file_action(".tmux.conf")],
                },
            ])
        );
    }

    #[test]
    fn include_a_source_when_to_exists_guard_uses_the_bare_tilde_default() {
        // When `to` is absent, the source inherits `defaults.to = "~"` (bare
        // tilde). Verifies that the bare tilde is expanded with the injected
        // home directory before the `to_exists` guard probes the filesystem.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(), // defaults.to = "~"
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("src".to_string()),
                to: None, // inherits default "~"
                guard: Some(Guard::ToExists),
                configs: vec![make_file_config_item(".zshrc")],
            }],
        };

        // The guard must expand "~" to "/home/tester" before the probe; if it
        // did not, `is_directory` would receive the literal "~" string, return
        // false, and the source would be excluded from the plan.
        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(
                &NoEnvVars,
                &FnDirProbe(|path| path == "/home/tester"),
                &RejectConfigRead,
            ),
        )
        .expect(
            "planning should succeed when to_exists guard uses the bare-tilde default destination",
        );

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "src".to_string(),
                actions: vec![expected_file_action(".zshrc")],
            }])
        );
    }

    #[test]
    fn include_sources_from_nested_includes_depth_first() {
        // Root includes /inc/a.yml; /inc/a.yml itself includes /inc/c.yml.
        // Expected order: root → a → c (depth-first, includes-after at each level).
        let c = make_config(vec![make_source(
            SourceRoot::Path("c_src".to_string()),
            vec![make_file_config_item(".bashrc")],
        )]);
        let a = Config {
            includes: vec![Include::Path {
                path: "/inc/c.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("a_src".to_string()),
                vec![make_file_config_item(".aliases")],
            )])
        };
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    if path == Path::new("/inc/a.yml") {
                        Ok(loaded_config_for_test(a.clone(), path))
                    } else {
                        assert_eq!(path, Path::new("/inc/c.yml"));
                        Ok(loaded_config_for_test(c.clone(), path))
                    }
                }),
            ),
        )
        .expect("planning should succeed with nested includes");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: "root_src".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "a_src".to_string(),
                    actions: vec![expected_file_action(".aliases")],
                },
                IntentSource {
                    from: "c_src".to_string(),
                    actions: vec![expected_file_action(".bashrc")],
                },
            ])
        );
    }

    #[test]
    fn skip_duplicate_includes_across_the_include_tree() {
        // Both root and /inc/a.yml include /inc/shared.yml.
        // /inc/shared.yml should only appear once in the plan.
        let shared = make_config(vec![make_source(
            SourceRoot::Path("shared_src".to_string()),
            vec![make_file_config_item(".shared")],
        )]);
        let a = Config {
            includes: vec![Include::Path {
                path: "/inc/shared.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("a_src".to_string()),
                vec![make_file_config_item(".aliases")],
            )])
        };
        let root = Config {
            includes: vec![
                Include::Path {
                    path: "/inc/a.yml".to_string(),
                    optional: false,
                },
                Include::Path {
                    path: "/inc/shared.yml".to_string(),
                    optional: false,
                },
            ],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    if path == Path::new("/inc/a.yml") {
                        Ok(loaded_config_for_test(a.clone(), path))
                    } else {
                        assert_eq!(path, Path::new("/inc/shared.yml"));
                        Ok(loaded_config_for_test(shared.clone(), path))
                    }
                }),
            ),
        )
        .expect("planning should succeed and deduplicate shared includes");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: "root_src".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "a_src".to_string(),
                    actions: vec![expected_file_action(".aliases")],
                },
                IntentSource {
                    from: "shared_src".to_string(),
                    actions: vec![expected_file_action(".shared")],
                },
            ])
        );
    }

    #[test]
    fn skip_optional_includes_when_file_is_missing() {
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/missing.yml".to_string(),
                optional: true,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    Err(ProgramError::ConfigurationReadFailed {
                        path: path.to_path_buf(),
                        message: "not found".to_string(),
                    })
                }),
            ),
        )
        .expect("planning should succeed when an optional include is missing");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: "root_src".to_string(),
                actions: vec![expected_file_action(".zshrc")],
            }])
        );
    }

    #[test]
    fn reject_required_includes_that_cannot_be_found() {
        let root = Config {
            includes: vec![
                Include::Path {
                    path: "/inc/missing_a.yml".to_string(),
                    optional: false,
                },
                Include::Path {
                    path: "/inc/missing_b.yml".to_string(),
                    optional: false,
                },
            ],
            ..make_config(vec![make_source(
                SourceRoot::Path(".".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let error = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    Err(ProgramError::ConfigurationReadFailed {
                        path: path.to_path_buf(),
                        message: "not found".to_string(),
                    })
                }),
            ),
        )
        .expect_err("planning should fail when required includes are missing");

        assert_eq!(
            error,
            ProgramError::IncludeNotFound {
                paths: vec![
                    PathBuf::from("/inc/missing_a.yml"),
                    PathBuf::from("/inc/missing_b.yml"),
                ],
            }
        );
    }

    #[test]
    fn reject_include_cycles() {
        // /inc/a.yml includes /inc/b.yml, which includes /inc/a.yml — a cycle.
        let b = Config {
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("b_src".to_string()),
                vec![make_file_config_item(".b")],
            )])
        };
        let a = Config {
            includes: vec![Include::Path {
                path: "/inc/b.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("a_src".to_string()),
                vec![make_file_config_item(".a")],
            )])
        };
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let error = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    if path == Path::new("/inc/a.yml") {
                        Ok(loaded_config_for_test(a.clone(), path))
                    } else {
                        assert_eq!(path, Path::new("/inc/b.yml"));
                        Ok(loaded_config_for_test(b.clone(), path))
                    }
                }),
            ),
        )
        .expect_err("planning should fail when an include cycle is detected");

        assert_eq!(
            error,
            ProgramError::IncludeCycle {
                cycle: vec![
                    PathBuf::from("/inc/a.yml"),
                    PathBuf::from("/inc/b.yml"),
                    PathBuf::from("/inc/a.yml"),
                ],
            }
        );
    }

    #[test]
    fn reject_a_self_referencing_include_when_root_config_path_is_seeded() {
        let root_path = Path::new("/inc/root.yml");
        let root = Config {
            includes: vec![Include::Path {
                path: root_path.display().to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path("root_src".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };
        let config_reader = FnConfigReader(|path: &Path| crate::load_config(path));

        let error = derive_intent_plan(
            &loaded_config_for_path(&root, Some(root_path)),
            &make_context(&NoEnvVars, &NoDirectories, &config_reader),
        )
        .expect_err("planning should fail when the root config includes itself");

        assert_eq!(
            error,
            ProgramError::IncludeCycle {
                cycle: vec![
                    PathBuf::from("/inc/root.yml"),
                    PathBuf::from("/inc/root.yml")
                ],
            }
        );
    }

    #[test]
    fn resolve_env_backed_include_paths_from_environment() {
        let included = make_config(vec![make_source(
            SourceRoot::Path("private_src".to_string()),
            vec![make_file_config_item(".secrets")],
        )]);
        let root = Config {
            includes: vec![Include::Environment {
                env: "PRIVATE_CONFIG".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path(".".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };
        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &FnEnvReader(|name: &str| {
                    assert_eq!(name, "PRIVATE_CONFIG");
                    Some("/private/.gitenv.yml".to_string())
                }),
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    assert_eq!(path, Path::new("/private/.gitenv.yml"));
                    Ok(loaded_config_for_test(included.clone(), path))
                }),
            ),
        )
        .expect("planning should resolve an environment-backed include path");

        assert_eq!(
            plan,
            expected_plan(vec![
                IntentSource {
                    from: ".".to_string(),
                    actions: vec![expected_file_action(".zshrc")],
                },
                IntentSource {
                    from: "private_src".to_string(),
                    actions: vec![expected_file_action(".secrets")],
                },
            ])
        );
    }

    #[test]
    fn skip_optional_env_backed_includes_when_variable_is_unset() {
        let root = Config {
            includes: vec![Include::Environment {
                env: "PRIVATE_CONFIG".to_string(),
                optional: true,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path(".".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&root),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should succeed when an optional env-backed include variable is unset");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![expected_file_action(".zshrc")],
            }])
        );
    }

    #[test]
    fn reject_required_env_backed_includes_when_variable_is_unset() {
        let root = Config {
            includes: vec![Include::Environment {
                env: "PRIVATE_CONFIG".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path(".".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let error = derive_intent_plan(
            &root_loaded_config(&root),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect_err("planning should fail when a required env-backed include variable is unset");

        assert_eq!(
            error,
            ProgramError::MissingEnvironment {
                vars: vec!["PRIVATE_CONFIG".to_string()],
            }
        );
    }

    #[test]
    fn apply_each_included_configs_defaults_only_to_its_own_sources() {
        // Root uses copy mode; the included config uses symlink mode.
        // Each should plan with its own defaults, not the other's.
        let included = Config {
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path("inc_src".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".tmux.conf".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
            ..Config {
                version: 1,
                repository: "~/projects/env".to_string(),
                defaults: Defaults::default(),
                sources: vec![],
                includes: vec![],
            }
        };
        let root = Config {
            defaults: Defaults {
                mode: ActionMode::Copy,
                to: "~/dest".to_string(),
                mkdir: false,
                overwrite: true,
                backup_on_overwrite: false,
            },
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
            sources: vec![Source {
                from: SourceRoot::Path("root_src".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
            ..Config {
                version: 1,
                repository: "~/projects/env".to_string(),
                defaults: Defaults::default(),
                sources: vec![],
                includes: vec![],
            }
        };

        let plan = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|path: &Path| {
                    assert_eq!(path, Path::new("/inc/a.yml"));
                    Ok(loaded_config_for_test(included.clone(), path))
                }),
            ),
        )
        .expect("planning should succeed and isolate defaults per included config");

        assert_eq!(
            plan,
            expected_plan(vec![
                // Root's source uses copy mode with root's defaults.
                IntentSource {
                    from: "root_src".to_string(),
                    actions: vec![IntentAction::File(IntentFileAction {
                        file: ".zshrc".to_string(),
                        as_name: ".zshrc".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/dest".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    })],
                },
                // Included source uses symlink mode with included config's defaults.
                IntentSource {
                    from: "inc_src".to_string(),
                    actions: vec![IntentAction::File(IntentFileAction {
                        file: ".tmux.conf".to_string(),
                        as_name: ".tmux.conf".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Symlink,
                            to: "~".to_string(),
                            mkdir: true,
                            conflict_policy: ConflictPolicy::Skip,
                        },
                    })],
                },
            ])
        );
    }

    #[test]
    fn cannot_derive_intent_plan_when_parsing_included_configs_fails() {
        // An include whose file returns a non-ConfigurationReadFailed error
        // (e.g. a structural parse failure) must be propagated as-is rather
        // than collected into an IncludeNotFound set.
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/bad.yml".to_string(),
                optional: false,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path(".".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let error = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|_| {
                    Err(ProgramError::InvalidConfiguration {
                        message: "unknown field `oops`".to_string(),
                    })
                }),
            ),
        )
        .expect_err("a parse error from an included config should propagate immediately");

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "unknown field `oops`".to_string(),
            }
        );
    }

    #[test]
    fn cannot_derive_intent_plan_when_parsing_optional_includes_fails() {
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/optional-bad.yml".to_string(),
                optional: true,
            }],
            ..make_config(vec![make_source(
                SourceRoot::Path(".".to_string()),
                vec![make_file_config_item(".zshrc")],
            )])
        };

        let error = derive_intent_plan(
            &loaded_config_for_path(&root, None),
            &make_context(
                &NoEnvVars,
                &NoDirectories,
                &FnConfigReader(|_| {
                    Err(ProgramError::InvalidConfiguration {
                        message: "unknown field `oops`".to_string(),
                    })
                }),
            ),
        )
        .expect_err(
            "planning should propagate structural include errors even when include is optional",
        );

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "unknown field `oops`".to_string(),
            }
        );
    }

    // ---------------------------------------------------------------------------
    // Item-level option overrides
    // ---------------------------------------------------------------------------

    #[test]
    fn item_level_mode_override_takes_precedence_over_default_mode() {
        // Global defaults use symlink mode; item overrides to copy.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: Some(ActionMode::Copy),
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should apply item-level mode override");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".zshrc".to_string(),
                    as_name: ".zshrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Copy,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn item_level_to_override_takes_precedence_over_source_to() {
        // Source-level `to` is ~/shared; item overrides to ~/item-specific.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: Some("~/shared".to_string()),
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: Some("~/item-specific".to_string()),
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should apply item-level to override");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".zshrc".to_string(),
                    as_name: ".zshrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~/item-specific".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn item_level_overwrite_true_resolves_to_overwrite_conflict_policy() {
        // Global defaults have overwrite disabled; item enables it.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: Some(true),
                    backup_on_overwrite: Some(false),
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should apply item-level overwrite override");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".zshrc".to_string(),
                    as_name: ".zshrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Overwrite,
                    },
                })],
            }])
        );
    }

    #[test]
    fn item_level_select_overrides_are_applied_independently() {
        // Select item overrides mode and to while inheriting everything else.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    selection_type: SelectionType::Dot,
                    recursive: false,
                    exclude: vec![],
                    mode: Some(ActionMode::Copy),
                    to: Some("~/config".to_string()),
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should apply select item overrides");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::Select(IntentSelectAction {
                    selection_type: SelectionType::Dot,
                    recursive: false,
                    exclude: vec![],
                    options: ResolvedOptions {
                        mode: ActionMode::Copy,
                        to: "~/config".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }

    #[test]
    fn cannot_set_overwrite_false_and_backup_on_overwrite_true_at_file_item_level() {
        // This combination is explicitly rejected when both are set at item level.
        // (Inherited defaults with the same values are fine — they resolve to Skip.)
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: Some(false),
                    backup_on_overwrite: Some(true),
                })],
            }],
        };

        let error = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect_err("planning should reject explicit overwrite: false with backup_on_overwrite: true at item level");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { ref message }
                if message.contains("overwrite: false") && message.contains("backup_on_overwrite: true")
        ));
    }

    #[test]
    fn cannot_set_overwrite_false_and_backup_on_overwrite_true_at_select_item_level() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    selection_type: SelectionType::Dot,
                    recursive: false,
                    exclude: vec![],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: Some(false),
                    backup_on_overwrite: Some(true),
                })],
            }],
        };

        let error = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect_err(
            "planning should reject explicit overwrite: false with backup_on_overwrite: true at select item level",
        );

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "item-level `overwrite: false` with `backup_on_overwrite: true` is not a valid combination"
                    .to_string(),
            }
        );
    }

    #[test]
    fn inherited_backup_default_is_valid_when_overwrite_resolves_to_false() {
        // Global defaults: overwrite: false, backup_on_overwrite: true (the defaults).
        // An item that doesn't override either should resolve to Skip without error.
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan(
            &root_loaded_config(&config),
            &make_context(&NoEnvVars, &NoDirectories, &RejectConfigRead),
        )
        .expect("planning should succeed when backup default is inherited and overwrite is false");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".zshrc".to_string(),
                    as_name: ".zshrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            }])
        );
    }
}
