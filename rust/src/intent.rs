use crate::ProgramError;
use crate::config::{
    ActionMode, Config, ConfigItem, Defaults, Guard, Include, LoadedConfig, SourceRoot,
};
use crate::load_config;
use crate::logging;
use log::Level;
use std::collections::{BTreeMap, BTreeSet};
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
/// selector intent (for example `dotfiles` and `exclude`) unexpanded.
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
    /// When `true`, dotfiles (names starting with `.`) are included.
    pub dotfiles: bool,
    /// Filenames explicitly excluded from selection.
    pub exclude: Vec<String>,
    pub options: ResolvedOptions,
}

/// Derives an intent plan from `config` using the real process environment and
/// real filesystem access.
///
/// This is the primary entry point for production use.
pub fn derive_intent_plan(loaded_config: &LoadedConfig) -> Result<IntentPlan, ProgramError> {
    let environment = std::env::vars().collect::<BTreeMap<_, _>>();
    derive_intent_plan_with_env(loaded_config, &environment)
}

/// Like `derive_intent_plan` but accepts an explicit environment map.
///
/// Useful in tests that need to control environment variables without mutating
/// the real process environment.
pub fn derive_intent_plan_with_env(
    loaded_config: &LoadedConfig,
    environment: &BTreeMap<String, String>,
) -> Result<IntentPlan, ProgramError> {
    derive_intent_plan_with_env_and_fs(loaded_config, environment, &|path| {
        logging::system(Level::Trace, "metadata", format!("path={path}"));
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    })
}

/// Core planning function. `is_directory` is injected so tests can evaluate
/// guards without touching the real filesystem.
///
/// Delegates to `derive_intent_plan_with_injectables` with the real
/// file-reader so include resolution works against actual config files.
pub fn derive_intent_plan_with_env_and_fs(
    loaded_config: &LoadedConfig,
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
) -> Result<IntentPlan, ProgramError> {
    derive_intent_plan_with_injectables(loaded_config, environment, is_directory, &load_config)
}

/// Fully injectable planning function used by tests to exercise include
/// resolution without writing real config files.
///
/// `read_config_file` is called once per include path. Tests supply a closure
/// that returns pre-parsed configs from an in-memory map.
pub fn derive_intent_plan_with_injectables(
    loaded_config: &LoadedConfig,
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
    read_config_file: &impl Fn(&Path) -> Result<LoadedConfig, ProgramError>,
) -> Result<IntentPlan, ProgramError> {
    let config: &Config = loaded_config;
    logging::intent(
        Level::Debug,
        "intent_plan_start",
        format!(
            "repository={} sources={} includes={} env_vars={}",
            config.repository,
            config.sources.len(),
            config.includes.len(),
            environment.len()
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
        environment,
        is_directory,
        read_config_file,
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

    let action_count = sources
        .iter()
        .map(|source| source.actions.len())
        .sum::<usize>();
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
#[allow(clippy::too_many_arguments)]
fn plan_sources_recursively(
    config: &Config,
    current_config_path: &Path,
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
    read_config_file: &impl Fn(&Path) -> Result<LoadedConfig, ProgramError>,
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
            } => match environment.get(env) {
                Some(value) => Some(value.clone()),
                None => {
                    missing_env.insert(env.clone());
                    None
                }
            },
            SourceRoot::Environment {
                env,
                optional: true,
            } => environment.get(env).cloned(),
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
        if let Some(guard) = &source.guard {
            let satisfied = match guard {
                Guard::ToExists => is_directory(&source_to),
                Guard::DirectoryExists(path) => is_directory(path),
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
                        dotfiles: select_config.dotfiles,
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
            Include::Path { path, optional } => (
                Some(resolve_include_path(path, current_config_path)),
                *optional,
            ),
            Include::Environment { env, optional } => match environment.get(env) {
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
        let loaded_included_config = match read_config_file(&include_path) {
            Ok(c) => c,
            // Optional includes are silently skipped when the file is missing.
            Err(ProgramError::ReadConfiguration { .. }) if optional => {
                seen.insert(include_path);
                continue;
            }
            // Required includes that cannot be read are collected for the
            // caller to surface as a single IncludeNotFound error.
            Err(ProgramError::ReadConfiguration { .. }) => {
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
            environment,
            is_directory,
            read_config_file,
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

fn resolve_include_path(path: &str, current_config_path: &Path) -> PathBuf {
    let include_path = PathBuf::from(path);
    if include_path.is_absolute() || path.starts_with('~') {
        return include_path;
    }

    current_config_path
        .parent()
        .map(|parent| parent.join(include_path))
        .unwrap_or_else(|| PathBuf::from(path))
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
        ConflictPolicy, IntentAction, IntentFileAction, IntentPlan, IntentSelectAction,
        IntentSource, ResolvedOptions, derive_intent_plan, derive_intent_plan_with_env,
        derive_intent_plan_with_env_and_fs, derive_intent_plan_with_injectables,
    };
    use crate::{
        ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
        ProgramError, SelectConfig, Source, SourceRoot,
    };
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

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
        let environment = BTreeMap::new();

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &environment)
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
                        dotfiles: true,
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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
                        dotfiles: true,
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
                from: SourceRoot::Path(".".to_string()),
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

        let plan = derive_intent_plan(&root_loaded_config(&config))
            .expect("planning should succeed when the current directory exists");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
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
                    optional: false,
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
        let environment = BTreeMap::from([(
            "PRIVATE_ENV_DIR".to_string(),
            "~/projects/private-env".to_string(),
        )]);

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &environment)
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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

        let error = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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
                from: SourceRoot::Path("vscode".to_string()),
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

        let plan = derive_intent_plan_with_env_and_fs(
            &root_loaded_config(&config),
            &BTreeMap::new(),
            &|_| false,
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
        // Simulate the destination directory being present.
        let known_dirs = std::collections::BTreeSet::from([
            "~/Library/Application Support/Code/User".to_string(),
        ]);

        let plan = derive_intent_plan_with_env_and_fs(
            &root_loaded_config(&config),
            &BTreeMap::new(),
            &|path| known_dirs.contains(path),
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
        // Destination directory is absent.
        let plan = derive_intent_plan_with_env_and_fs(
            &root_loaded_config(&config),
            &BTreeMap::new(),
            &|_| false,
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

        let plan = derive_intent_plan_with_env_and_fs(
            &root_loaded_config(&config),
            &BTreeMap::new(),
            &|path| known_dirs.contains(path),
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

        let plan = derive_intent_plan_with_env_and_fs(
            &root_loaded_config(&config),
            &BTreeMap::new(),
            &|_| false,
        )
        .expect("planning should succeed when the directory_exists guard is not satisfied");

        assert_eq!(plan, expected_plan(vec![]));
    }

    // ---------------------------------------------------------------------------
    // Conflict policy resolution
    // ---------------------------------------------------------------------------

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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/inc/a.yml"));
                Ok(loaded_config_for_test(included.clone(), path))
            },
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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, Some(root_path)),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/configs/inc/a.yml"));
                Ok(loaded_config_for_test(included.clone(), path))
            },
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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, Some(root_path)),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/configs/inc/a.yml") {
                    Ok(loaded_config_for_test(a.clone(), path))
                } else {
                    assert_eq!(path, Path::new("/configs/inc/nested/c.yml"));
                    Ok(loaded_config_for_test(c.clone(), path))
                }
            },
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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/inc/a.yml") {
                    Ok(loaded_config_for_test(a.clone(), path))
                } else {
                    assert_eq!(path, Path::new("/inc/c.yml"));
                    Ok(loaded_config_for_test(c.clone(), path))
                }
            },
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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/inc/a.yml") {
                    Ok(loaded_config_for_test(a.clone(), path))
                } else {
                    assert_eq!(path, Path::new("/inc/shared.yml"));
                    Ok(loaded_config_for_test(shared.clone(), path))
                }
            },
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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                Err(ProgramError::ReadConfiguration {
                    path: path.to_path_buf(),
                    message: "not found".to_string(),
                })
            },
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

        let error = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                Err(ProgramError::ReadConfiguration {
                    path: path.to_path_buf(),
                    message: "not found".to_string(),
                })
            },
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

        let error = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/inc/a.yml") {
                    Ok(loaded_config_for_test(a.clone(), path))
                } else {
                    assert_eq!(path, Path::new("/inc/b.yml"));
                    Ok(loaded_config_for_test(b.clone(), path))
                }
            },
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

        let error = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, Some(root_path)),
            &BTreeMap::new(),
            &|_| false,
            &crate::load_config,
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
        let environment = BTreeMap::from([(
            "PRIVATE_CONFIG".to_string(),
            "/private/.gitenv.yml".to_string(),
        )]);

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &environment,
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/private/.gitenv.yml"));
                Ok(loaded_config_for_test(included.clone(), path))
            },
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&root), &BTreeMap::new())
            .expect(
                "planning should succeed when an optional env-backed include variable is unset",
            );

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

        let error = derive_intent_plan_with_env(&root_loaded_config(&root), &BTreeMap::new())
            .expect_err(
                "planning should fail when a required env-backed include variable is unset",
            );

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

        let plan = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/inc/a.yml"));
                Ok(loaded_config_for_test(included.clone(), path))
            },
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
    fn propagate_parse_errors_from_included_configs() {
        // An include whose file returns a non-ReadConfiguration error (e.g. a
        // structural parse failure) must be propagated as-is rather than
        // collected into an IncludeNotFound set.
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

        let error = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|_| {
                Err(ProgramError::InvalidConfiguration {
                    message: "unknown field `oops`".to_string(),
                })
            },
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
    fn propagate_parse_errors_from_optional_includes() {
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

        let error = derive_intent_plan_with_injectables(
            &loaded_config_for_path(&root, None),
            &BTreeMap::new(),
            &|_| false,
            &|_| {
                Err(ProgramError::InvalidConfiguration {
                    message: "unknown field `oops`".to_string(),
                })
            },
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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

        let plan = derive_intent_plan_with_env_and_fs(
            &root_loaded_config(&config),
            &BTreeMap::new(),
            &|_| false,
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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
                    dotfiles: true,
                    exclude: vec![],
                    mode: Some(ActionMode::Copy),
                    to: Some("~/config".to_string()),
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
            .expect("planning should apply select item overrides");

        assert_eq!(
            plan,
            expected_plan(vec![IntentSource {
                from: ".".to_string(),
                actions: vec![IntentAction::Select(IntentSelectAction {
                    dotfiles: true,
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

        let error = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
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
                    dotfiles: true,
                    exclude: vec![],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: Some(false),
                    backup_on_overwrite: Some(true),
                })],
            }],
        };

        let error =
            derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new()).expect_err(
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

        let plan = derive_intent_plan_with_env(&root_loaded_config(&config), &BTreeMap::new())
            .expect(
                "planning should succeed when backup default is inherited and overwrite is false",
            );

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
