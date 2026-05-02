use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub repository: String,
    #[serde(default)]
    pub defaults: Defaults,
    pub sources: Vec<Source>,
    #[serde(default)]
    pub includes: Vec<Include>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub mode: ActionMode,
    pub to: String,
    pub mkdir: bool,
    pub overwrite: bool,
    pub backup_on_overwrite: bool,
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            mode: ActionMode::Symlink,
            to: "~".to_string(),
            mkdir: true,
            overwrite: false,
            backup_on_overwrite: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    Symlink,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictPolicy {
    Skip,
    Overwrite,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub from: SourceRoot,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(rename = "when")]
    pub guard: Option<Guard>,
    pub configs: Vec<ConfigItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRoot {
    Path(String),
    ExplicitPath { path: String },
    Environment { env: String, optional: bool },
}

impl<'de> Deserialize<'de> for SourceRoot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum SourceRootWire {
            Shorthand(String),
            Path {
                path: String,
            },
            Environment {
                env: String,
                #[serde(default)]
                optional: bool,
            },
        }

        match SourceRootWire::deserialize(deserializer)? {
            SourceRootWire::Shorthand(path) => {
                if let Some(env) = path.strip_prefix('$') {
                    if env.is_empty() {
                        Ok(Self::Path(path))
                    } else {
                        Ok(Self::Environment {
                            env: env.to_string(),
                            optional: false,
                        })
                    }
                } else {
                    Ok(Self::Path(path))
                }
            }
            SourceRootWire::Path { path } => Ok(Self::ExplicitPath { path }),
            SourceRootWire::Environment { env, optional } => {
                Ok(Self::Environment { env, optional })
            }
        }
    }
}

/// A declarative condition that must be satisfied for a source to be included in the plan.
///
/// `ToExists` checks that the resolved destination directory for this source exists on
/// the filesystem. This avoids repeating the destination path in both a `to` key and an
/// explicit path guard.
///
/// `DirectoryExists` checks an arbitrary path — useful for platform or tool detection
/// where the path has no relationship to the source's destination (e.g. `/Applications`
/// to detect macOS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guard {
    ToExists,
    DirectoryExists(String),
}

impl<'de> Deserialize<'de> for Guard {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum GuardWire {
            Shorthand(String),
            DirectoryExists { directory_exists: String },
        }

        match GuardWire::deserialize(deserializer)? {
            GuardWire::Shorthand(s) if s == "to_exists" => Ok(Self::ToExists),
            GuardWire::Shorthand(s) => Err(serde::de::Error::custom(format!(
                "unknown guard shorthand `{s}`, expected `to_exists`"
            ))),
            GuardWire::DirectoryExists { directory_exists } => {
                Ok(Self::DirectoryExists(directory_exists))
            }
        }
    }
}

/// A reference to another config file to compose into this one.
///
/// Shorthand string forms:
/// - `"~/path/to/file.yml"` — required literal path include.
/// - `"$VAR"` — required environment-backed path include.
///
/// Canonical forms accept an `optional` flag to silently skip when the
/// file or environment variable is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Include {
    Path { path: String, optional: bool },
    Environment { env: String, optional: bool },
}

impl<'de> Deserialize<'de> for Include {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum IncludeWire {
            Shorthand(String),
            Path {
                path: String,
                #[serde(default)]
                optional: bool,
            },
            Environment {
                env: String,
                #[serde(default)]
                optional: bool,
            },
        }

        match IncludeWire::deserialize(deserializer)? {
            IncludeWire::Shorthand(s) => {
                if let Some(env) = s.strip_prefix('$') {
                    if env.is_empty() {
                        Ok(Self::Path {
                            path: s,
                            optional: false,
                        })
                    } else {
                        Ok(Self::Environment {
                            env: env.to_string(),
                            optional: false,
                        })
                    }
                } else {
                    Ok(Self::Path {
                        path: s,
                        optional: false,
                    })
                }
            }
            IncludeWire::Path { path, optional } => Ok(Self::Path { path, optional }),
            IncludeWire::Environment { env, optional } => Ok(Self::Environment { env, optional }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigItem {
    File(FileConfig),
    Select(SelectConfig),
}

impl<'de> Deserialize<'de> for ConfigItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ConfigItemWire {
            ShorthandFile(String),
            File(FileConfig),
            Select { select: SelectConfig },
        }

        match ConfigItemWire::deserialize(deserializer)? {
            ConfigItemWire::ShorthandFile(file) => Ok(Self::File(FileConfig {
                file,
                as_name: None,
                mode: None,
                to: None,
                mkdir: None,
                overwrite: None,
                backup_on_overwrite: None,
            })),
            ConfigItemWire::File(file_config) => Ok(Self::File(file_config)),
            ConfigItemWire::Select { select } => Ok(Self::Select(select)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    pub file: String,
    #[serde(rename = "as")]
    pub as_name: Option<String>,
    #[serde(default)]
    pub mode: Option<ActionMode>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub mkdir: Option<bool>,
    #[serde(default)]
    pub overwrite: Option<bool>,
    #[serde(default)]
    pub backup_on_overwrite: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectConfig {
    pub dotfiles: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub mode: Option<ActionMode>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub mkdir: Option<bool>,
    #[serde(default)]
    pub overwrite: Option<bool>,
    #[serde(default)]
    pub backup_on_overwrite: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    InvalidConfiguration {
        message: String,
    },
    ReadConfiguration {
        path: PathBuf,
        message: String,
    },
    MissingEnvironment {
        vars: Vec<String>,
    },
    /// One or more required include files could not be found. The paths are
    /// sorted for deterministic output.
    IncludeNotFound {
        paths: Vec<PathBuf>,
    },
    /// A config file was encountered more than once in the same include chain,
    /// forming a cycle. Contains the full cycle path in traversal order,
    /// ending with the repeated path that closes the cycle.
    IncludeCycle {
        cycle: Vec<PathBuf>,
    },
    UnsupportedConfiguration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub repository: String,
    pub sources: Vec<PlannedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSource {
    pub from: String,
    pub actions: Vec<PlannedAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedAction {
    File(PlannedFileAction),
    Select(PlannedSelectAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFileAction {
    pub file: String,
    pub as_name: String,
    pub options: ResolvedOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSelectAction {
    pub dotfiles: bool,
    pub exclude: Vec<String>,
    pub options: ResolvedOptions,
}

pub fn parse_config(yaml: &str) -> Result<Config, ProgramError> {
    serde_yaml::from_str(yaml).map_err(|error| ProgramError::InvalidConfiguration {
        message: format!("failed to parse config YAML: {error}"),
    })
}

pub fn load_config(path: &Path) -> Result<Config, ProgramError> {
    let yaml = std::fs::read_to_string(path).map_err(|error| ProgramError::ReadConfiguration {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;

    parse_config(&yaml)
}

pub fn derive_execution_plan(config: &Config) -> Result<ExecutionPlan, ProgramError> {
    let environment = std::env::vars().collect::<BTreeMap<_, _>>();
    derive_execution_plan_with_env(config, &environment)
}

pub fn derive_execution_plan_with_env(
    config: &Config,
    environment: &BTreeMap<String, String>,
) -> Result<ExecutionPlan, ProgramError> {
    derive_execution_plan_with_env_and_fs(config, environment, &|path| {
        std::fs::metadata(path).map(|m| m.is_dir()).unwrap_or(false)
    })
}

/// Core planning function. `is_directory` is injected so tests can evaluate
/// guards without touching the real filesystem.
///
/// Delegates to `derive_execution_plan_with_injectables` with the real
/// file-reader so include resolution works against actual config files.
pub fn derive_execution_plan_with_env_and_fs(
    config: &Config,
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
) -> Result<ExecutionPlan, ProgramError> {
    derive_execution_plan_with_injectables(config, None, environment, is_directory, &load_config)
}

/// Fully injectable planning function used by tests to exercise include
/// resolution without writing real config files.
///
/// `config_path` is the canonical path of the root config file, used to seed
/// the cycle-detection stack. Pass `None` when there is no file on disk (e.g.
/// configs built in tests via `parse_config`).
///
/// `read_config_file` is called once per include path. Tests supply a closure
/// that returns pre-parsed configs from an in-memory map.
pub fn derive_execution_plan_with_injectables(
    config: &Config,
    config_path: Option<&Path>,
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
    read_config_file: &impl Fn(&Path) -> Result<Config, ProgramError>,
) -> Result<ExecutionPlan, ProgramError> {
    let mut missing_env: BTreeSet<String> = BTreeSet::new();
    let mut missing_files: BTreeSet<PathBuf> = BTreeSet::new();
    // in_flight tracks the ancestor chain for cycle detection; seed with the
    // root config path so a self-referencing include is caught.
    let mut in_flight: Vec<PathBuf> = config_path
        .map(|p| vec![p.to_path_buf()])
        .unwrap_or_default();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();

    let sources = plan_sources_recursively(
        config,
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

    Ok(ExecutionPlan {
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
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
    read_config_file: &impl Fn(&Path) -> Result<Config, ProgramError>,
    in_flight: &mut Vec<PathBuf>,
    seen: &mut BTreeSet<PathBuf>,
    missing_env: &mut BTreeSet<String>,
    missing_files: &mut BTreeSet<PathBuf>,
) -> Result<Vec<PlannedSource>, ProgramError> {
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
                    Ok(PlannedAction::File(PlannedFileAction {
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
                    Ok(PlannedAction::Select(PlannedSelectAction {
                        dotfiles: select_config.dotfiles,
                        exclude: select_config.exclude.clone(),
                        options,
                    }))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        own_sources.push(PlannedSource { from, actions });
    }

    // Resolve and plan each include, appending their sources after own sources.
    let mut included_sources = Vec::new();
    for include in &config.includes {
        let (include_path, optional) = match include {
            Include::Path { path, optional } => (Some(PathBuf::from(path)), *optional),
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
        let included_config = match read_config_file(&include_path) {
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
            // immediately — they indicate a malformed config, not a
            // missing file.
            Err(e) => return Err(e),
        };

        // Push to in_flight before recursing to make this path visible to
        // cycle detection in the subtree. Seen is updated only after full
        // processing completes so that in-flight ancestors are never mistaken
        // for already-finished (deduplicated) nodes.
        in_flight.push(include_path.clone());

        let sub_sources = plan_sources_recursively(
            &included_config,
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

pub fn run() -> Result<ProgramOutput, ProgramError> {
    Ok(ProgramOutput {
        message: "Hello, World!",
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ActionMode, Config, ConfigItem, ConflictPolicy, Defaults, ExecutionPlan, FileConfig, Guard,
        Include, PlannedAction, PlannedFileAction, PlannedSelectAction, PlannedSource,
        ProgramError, ResolvedOptions, SelectConfig, Source, SourceRoot, derive_execution_plan,
        derive_execution_plan_with_env, derive_execution_plan_with_env_and_fs,
        derive_execution_plan_with_injectables, load_config, parse_config, run,
    };
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn the_default_action_is_a_home_symlink() {
        let defaults = Defaults::default();

        assert_eq!(defaults.mode, ActionMode::Symlink);
        assert_eq!(defaults.to, "~");
        assert!(defaults.mkdir);
        assert!(!defaults.overwrite);
        assert!(defaults.backup_on_overwrite);
    }

    #[test]
    fn create_a_canonical_config() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
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
                    ConfigItem::Select(SelectConfig {
                        dotfiles: true,
                        exclude: vec![".DS_Store".to_string()],
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                ],
            }],
        };

        assert_eq!(config.version, 1);
        assert_eq!(config.repository, "~/projects/env");
        assert_eq!(config.sources.len(), 1);
    }

    #[test]
    fn show_the_default_message() {
        let output = run().expect("run should succeed");

        assert_eq!(output.message, "Hello, World!");
    }

    #[test]
    fn load_the_smallest_valid_config() {
        let file_path = unique_temp_file_path("smallest_valid_config");
        let yaml = r#"
version: 1
repository: ~/projects/env
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
sources:
  - from: "."
    configs:
      - file: .zshrc
"#;

        std::fs::write(&file_path, yaml).expect("temp config should be written");
        let config = load_config(&file_path).expect("config should load");
        std::fs::remove_file(&file_path).expect("temp config should be removed");

        let expected = Config {
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

        assert_eq!(config, expected);
    }

    #[test]
    fn accept_a_config_without_a_defaults_section() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - file: .zshrc
"#;

        let config = parse_config(yaml).expect("config should parse without defaults");

        assert_eq!(config.defaults, Defaults::default());
    }

    #[test]
    fn accept_a_config_with_partial_defaults() {
        let yaml = r#"
version: 1
repository: ~/projects/env
defaults:
  to: "~/.config"
  overwrite: true
sources:
  - from: "."
    configs:
      - file: .zshrc
"#;

        let config = parse_config(yaml).expect("config should parse with partial defaults");

        let expected_defaults = Defaults {
            mode: ActionMode::Symlink,
            to: "~/.config".to_string(),
            mkdir: true,
            overwrite: true,
            backup_on_overwrite: true,
        };

        assert_eq!(config.defaults, expected_defaults);
    }

    #[test]
    fn accept_select_entries_in_source_configs() {
        let yaml = r#"
version: 1
repository: ~/projects/env
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
sources:
  - from: "."
    configs:
      - select:
          dotfiles: true
          exclude:
            - .DS_Store
            - .git
"#;

        let config = parse_config(yaml).expect("config should parse with a select item");

        let expected = Config {
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
                    exclude: vec![".DS_Store".to_string(), ".git".to_string()],
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        assert_eq!(config, expected);
    }

    #[test]
    fn accept_shorthand_file_entries_in_source_configs() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
"#;

        let config = parse_config(yaml).expect("config should parse with shorthand file entries");

        let expected = Config {
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
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
        };

        assert_eq!(config, expected);
    }

    #[test]
    fn normalize_shorthand_and_canonical_configs_to_the_same_model() {
        let shorthand_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
      - file: .tmux
        as: .tmux.conf
      - select:
          dotfiles: true
"#;
        let canonical_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - file: .zshrc
      - file: .tmux
        as: .tmux.conf
      - select:
          dotfiles: true
          exclude: []
"#;

        let shorthand = parse_config(shorthand_yaml)
            .expect("shorthand config should parse to the normalized model");
        let canonical = parse_config(canonical_yaml)
            .expect("canonical config should parse to the normalized model");

        assert_eq!(shorthand, canonical);
    }

    #[test]
    fn derive_a_deterministic_execution_plan_from_a_config() {
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("config should produce an execution plan");

        let expected = ExecutionPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![PlannedSource {
                from: ".".to_string(),
                actions: vec![
                    PlannedAction::File(PlannedFileAction {
                        file: ".zshrc".to_string(),
                        as_name: ".zshrc".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/dest".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    }),
                    PlannedAction::File(PlannedFileAction {
                        file: ".tmux".to_string(),
                        as_name: ".tmux.conf".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/dest".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    }),
                    PlannedAction::Select(PlannedSelectAction {
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
    fn accept_environment_backed_source_roots() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from:
      env: PRIVATE_ENV_DIR
    configs:
      - file: .zshrc
"#;

        let config =
            parse_config(yaml).expect("config should parse with an environment-backed source root");

        let expected = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
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
            }],
        };

        assert_eq!(config, expected);
    }

    #[test]
    fn interpret_dollar_prefixed_shorthand_source_roots_as_environment_sources() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "$PRIVATE_ENV_DIR"
    configs:
      - file: .zshrc
"#;

        let config = parse_config(yaml).expect(
            "config should parse dollar-prefixed shorthand source roots as environment sources",
        );

        let expected = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
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
            }],
        };

        assert_eq!(config, expected);
    }

    #[test]
    fn accept_explicit_path_source_roots_that_start_with_a_dollar_sign() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from:
      path: "$PRIVATE_ENV_DIR"
    configs:
      - file: .zshrc
"#;

        let config = parse_config(yaml)
            .expect("config should parse explicit path source roots that start with a dollar sign");

        let expected = Config {
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

        assert_eq!(config, expected);
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

        let plan = derive_execution_plan_with_env(&config, &environment)
            .expect("planning should resolve the environment-backed source");

        let expected = ExecutionPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![PlannedSource {
                from: "~/projects/private-env".to_string(),
                actions: vec![PlannedAction::File(PlannedFileAction {
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should collapse overwrite-disabled defaults to skip");

        let expected = ExecutionPlan {
            repository: "~/projects/env".to_string(),
            sources: vec![PlannedSource {
                from: ".".to_string(),
                actions: vec![PlannedAction::File(PlannedFileAction {
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

        let error = derive_execution_plan_with_env(&config, &BTreeMap::new())
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should succeed when an optional environment-backed source is unset");

        assert_eq!(plan.repository, "~/projects/env");
        assert!(
            plan.sources.is_empty(),
            "unset optional sources should be omitted from the plan"
        );
    }

    #[test]
    fn cannot_load_a_missing_config_file() {
        let file_path = unique_temp_file_path("missing_config");

        let error = load_config(&file_path).expect_err("missing file should fail to load");

        assert!(matches!(
            &error,
            ProgramError::ReadConfiguration { path, message }
                if path == &file_path && !message.is_empty()
        ));
    }

    #[test]
    fn reject_a_config_missing_a_repository() {
        let yaml = r#"
version: 1
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
sources:
  - from: "."
    configs:
      - file: .zshrc
"#;

        let error = parse_config(yaml).expect_err("config should fail without repository");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { message }
                if message.contains("missing field `repository`")
        ));
    }

    #[test]
    fn reject_a_malformed_config() {
        let yaml = "version: [";

        let error = parse_config(yaml).expect_err("malformed yaml should fail");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { message }
                if message.contains("failed to parse config YAML")
        ));
    }

    #[test]
    fn reject_a_config_with_an_unknown_top_level_key() {
        let yaml = r#"
version: 1
repository: ~/projects/env
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
sources:
  - from: "."
    configs:
      - file: .zshrc
unknown: true
"#;

        let error = parse_config(yaml).expect_err("config should fail on unknown top-level key");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { message }
                if message.contains("unknown field `unknown`")
        ));
    }

    #[test]
    fn reject_a_config_with_an_unknown_nested_key() {
        let yaml = r#"
version: 1
repository: ~/projects/env
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
  extra: true
sources:
  - from: "."
    configs:
      - file: .zshrc
"#;

        let error = parse_config(yaml).expect_err("config should fail on unknown nested key");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { message }
                if message.contains("unknown field `extra`")
        ));
    }

    #[test]
    fn load_a_config_with_a_to_exists_guard() {
        let yaml = r#"
    version: 1
    repository: ~/projects/env
    sources:
      - from: vscode
        to: ~/Library/Application Support/Code/User
        when: to_exists
        configs:
          - keybindings.json
    "#;

        let config = parse_config(yaml).expect("config should parse with a to_exists guard");

        let source = &config.sources[0];
        assert_eq!(
            source.to.as_deref(),
            Some("~/Library/Application Support/Code/User")
        );
        assert_eq!(source.guard, Some(Guard::ToExists));
    }

    #[test]
    fn load_a_config_with_a_directory_exists_guard() {
        let yaml = r#"
    version: 1
    repository: ~/projects/env
    sources:
      - from: macos
        when:
          directory_exists: /Applications
        configs:
          - .macos-defaults
    "#;

        let config = parse_config(yaml).expect("config should parse with a directory_exists guard");

        let source = &config.sources[0];
        assert_eq!(
            source.guard,
            Some(Guard::DirectoryExists("/Applications".to_string()))
        );
    }

    #[test]
    fn reject_an_unknown_guard_shorthand() {
        let yaml = r#"
    version: 1
    repository: ~/projects/env
    sources:
      - from: .
        when: something_unsupported
        configs:
          - .zshrc
    "#;

        let error =
            parse_config(yaml).expect_err("config should reject an unknown guard shorthand");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { message }
                if message.contains("unknown guard shorthand")
        ));
    }

    #[test]
    fn load_a_config_with_a_source_level_to() {
        let yaml = r#"
    version: 1
    repository: ~/projects/env
    sources:
      - from: vscode
        to: ~/Library/Application Support/Code/User
        configs:
          - settings.json
    "#;

        let config = parse_config(yaml).expect("config should parse with a source-level to");

        assert_eq!(
            config.sources[0].to.as_deref(),
            Some("~/Library/Application Support/Code/User")
        );
    }

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

        let plan = derive_execution_plan_with_env_and_fs(&config, &BTreeMap::new(), &|_| false)
            .expect("planning should succeed with a source-level to");

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: "settings.json".to_string(),
                as_name: "settings.json".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Symlink,
                    to: "~/Library/Application Support/Code/User".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            })
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

        let plan = derive_execution_plan_with_env_and_fs(&config, &BTreeMap::new(), &|path| {
            known_dirs.contains(path)
        })
        .expect("planning should succeed when the to_exists guard is satisfied");

        assert_eq!(plan.sources.len(), 1, "source should be included");
        assert_eq!(plan.sources[0].from, "vscode");
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
        let plan = derive_execution_plan_with_env_and_fs(&config, &BTreeMap::new(), &|_| false)
            .expect("planning should succeed when the to_exists guard is not satisfied");

        assert!(
            plan.sources.is_empty(),
            "source should be excluded when guard is not satisfied"
        );
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

        let plan = derive_execution_plan_with_env_and_fs(&config, &BTreeMap::new(), &|path| {
            known_dirs.contains(path)
        })
        .expect("planning should succeed when the directory_exists guard is satisfied");

        assert_eq!(plan.sources.len(), 1, "source should be included");
        assert_eq!(plan.sources[0].from, "macos");
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

        let plan = derive_execution_plan_with_env_and_fs(&config, &BTreeMap::new(), &|_| false)
            .expect("planning should succeed when the directory_exists guard is not satisfied");

        assert!(
            plan.sources.is_empty(),
            "source should be excluded when guard is not satisfied"
        );
    }

    fn unique_temp_file_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("gitenv_{prefix}_{nanos}.yml"))
    }

    // ---------------------------------------------------------------------------
    // Include parsing tests
    // ---------------------------------------------------------------------------

    #[test]
    fn accept_includes_as_shorthand_literal_paths() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
includes:
  - ~/projects/private-env/.gitenv.yml
"#;

        let config = parse_config(yaml).expect("config should parse with a shorthand include");

        assert_eq!(
            config.includes,
            vec![Include::Path {
                path: "~/projects/private-env/.gitenv.yml".to_string(),
                optional: false,
            }]
        );
    }

    #[test]
    fn accept_includes_as_shorthand_dollar_prefixed_env_paths() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
includes:
  - $WORK_ENV_CONFIG
"#;

        let config =
            parse_config(yaml).expect("config should parse with a dollar-prefixed include");

        assert_eq!(
            config.includes,
            vec![Include::Environment {
                env: "WORK_ENV_CONFIG".to_string(),
                optional: false,
            }]
        );
    }

    #[test]
    fn accept_canonical_path_includes_with_optional_flag() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
includes:
  - path: ~/projects/work-env/.gitenv.yml
    optional: true
"#;

        let config =
            parse_config(yaml).expect("config should parse with a canonical optional include");

        assert_eq!(
            config.includes,
            vec![Include::Path {
                path: "~/projects/work-env/.gitenv.yml".to_string(),
                optional: true,
            }]
        );
    }

    #[test]
    fn accept_canonical_env_backed_includes_with_optional_flag() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
includes:
  - env: EXTRA_ENV_CONFIG
    optional: true
"#;

        let config =
            parse_config(yaml).expect("config should parse with a canonical optional env include");

        assert_eq!(
            config.includes,
            vec![Include::Environment {
                env: "EXTRA_ENV_CONFIG".to_string(),
                optional: true,
            }]
        );
    }

    #[test]
    fn accept_a_config_without_an_includes_section() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
"#;

        let config = parse_config(yaml).expect("config should parse without includes");

        assert!(config.includes.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Include planning tests
    //
    // These tests use `derive_execution_plan_with_injectables` and supply a
    // `read_config_file` closure that returns pre-built in-memory configs keyed
    // by path string, so no real files need to be written.
    // ---------------------------------------------------------------------------

    /// Returns a minimal config with one file source rooted at `from`.
    fn single_source_config(from: &str, file: &str) -> Config {
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![Source {
                from: SourceRoot::Path(from.to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: file.to_string(),
                    as_name: None,
                    mode: None,
                    to: None,
                    mkdir: None,
                    overwrite: None,
                    backup_on_overwrite: None,
                })],
            }],
            includes: vec![],
        }
    }

    /// Minimal planned file action with symlink/skip defaults pointing to `~`.
    fn expected_file_action(file: &str) -> PlannedAction {
        PlannedAction::File(PlannedFileAction {
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

    #[test]
    fn include_declared_files_sources_after_own_sources() {
        // Root declares .zshrc; includes /inc/a.yml which declares .tmux.conf.
        // Expected order: root's source first, then the include's source.
        let included = single_source_config("inc", ".tmux.conf");
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

        let plan = derive_execution_plan_with_injectables(
            &root,
            None,
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/inc/a.yml"));
                Ok(included.clone())
            },
        )
        .expect("planning should succeed with a valid include");

        assert_eq!(plan.sources.len(), 2);
        assert_eq!(plan.sources[0].from, ".");
        assert_eq!(
            plan.sources[0].actions,
            vec![expected_file_action(".zshrc")]
        );
        assert_eq!(plan.sources[1].from, "inc");
        assert_eq!(
            plan.sources[1].actions,
            vec![expected_file_action(".tmux.conf")]
        );
    }

    #[test]
    fn include_sources_from_nested_includes_depth_first() {
        // Root includes /inc/a.yml; /inc/a.yml itself includes /inc/c.yml.
        // Expected order: root → a → c (depth-first, includes-after at each level).
        let c = single_source_config("c_src", ".bashrc");
        let a = Config {
            includes: vec![Include::Path {
                path: "/inc/c.yml".to_string(),
                optional: false,
            }],
            ..single_source_config("a_src", ".aliases")
        };
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
            ..single_source_config("root_src", ".zshrc")
        };

        let plan = derive_execution_plan_with_injectables(
            &root,
            None,
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/inc/a.yml") {
                    Ok(a.clone())
                } else {
                    assert_eq!(path, Path::new("/inc/c.yml"));
                    Ok(c.clone())
                }
            },
        )
        .expect("planning should succeed with nested includes");

        assert_eq!(plan.sources.len(), 3);
        // root_src comes first (includes-after), then a_src, then c_src.
        assert_eq!(plan.sources[0].from, "root_src");
        assert_eq!(plan.sources[1].from, "a_src");
        assert_eq!(plan.sources[2].from, "c_src");
    }

    #[test]
    fn skip_duplicate_includes_across_the_include_tree() {
        // Both root and /inc/a.yml include /inc/shared.yml.
        // /inc/shared.yml should only appear once in the plan.
        let shared = single_source_config("shared_src", ".shared");
        let a = Config {
            includes: vec![Include::Path {
                path: "/inc/shared.yml".to_string(),
                optional: false,
            }],
            ..single_source_config("a_src", ".aliases")
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
            ..single_source_config("root_src", ".zshrc")
        };

        let plan = derive_execution_plan_with_injectables(
            &root,
            None,
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/inc/a.yml") {
                    Ok(a.clone())
                } else {
                    assert_eq!(path, Path::new("/inc/shared.yml"));
                    Ok(shared.clone())
                }
            },
        )
        .expect("planning should succeed and deduplicate shared includes");

        // root_src, a_src, shared_src — shared only once despite two references.
        let froms: Vec<&str> = plan.sources.iter().map(|s| s.from.as_str()).collect();
        assert_eq!(froms, vec!["root_src", "a_src", "shared_src"]);
    }

    #[test]
    fn skip_optional_includes_when_file_is_missing() {
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/missing.yml".to_string(),
                optional: true,
            }],
            ..single_source_config("root_src", ".zshrc")
        };

        let plan = derive_execution_plan_with_injectables(
            &root,
            None,
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

        // Only the root's own source is present; the missing optional include is silently skipped.
        assert_eq!(plan.sources.len(), 1);
        assert_eq!(plan.sources[0].from, "root_src");
    }

    #[test]
    fn reject_missing_required_includes() {
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
            ..single_source_config(".", ".zshrc")
        };

        let error = derive_execution_plan_with_injectables(
            &root,
            None,
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
            ..single_source_config("b_src", ".b")
        };
        let a = Config {
            includes: vec![Include::Path {
                path: "/inc/b.yml".to_string(),
                optional: false,
            }],
            ..single_source_config("a_src", ".a")
        };
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/a.yml".to_string(),
                optional: false,
            }],
            ..single_source_config("root_src", ".zshrc")
        };

        let error = derive_execution_plan_with_injectables(
            &root,
            None,
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                if path == Path::new("/inc/a.yml") {
                    Ok(a.clone())
                } else {
                    assert_eq!(path, Path::new("/inc/b.yml"));
                    Ok(b.clone())
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
    fn resolve_env_backed_include_paths_from_environment() {
        let included = single_source_config("private_src", ".secrets");
        let root = Config {
            includes: vec![Include::Environment {
                env: "PRIVATE_CONFIG".to_string(),
                optional: false,
            }],
            ..single_source_config(".", ".zshrc")
        };
        let environment = BTreeMap::from([(
            "PRIVATE_CONFIG".to_string(),
            "/private/.gitenv.yml".to_string(),
        )]);

        let plan = derive_execution_plan_with_injectables(
            &root,
            None,
            &environment,
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/private/.gitenv.yml"));
                Ok(included.clone())
            },
        )
        .expect("planning should resolve an environment-backed include path");

        assert_eq!(plan.sources.len(), 2);
        assert_eq!(plan.sources[1].from, "private_src");
    }

    #[test]
    fn skip_optional_env_backed_includes_when_variable_is_unset() {
        let root = Config {
            includes: vec![Include::Environment {
                env: "PRIVATE_CONFIG".to_string(),
                optional: true,
            }],
            ..single_source_config(".", ".zshrc")
        };

        let plan = derive_execution_plan_with_env(&root, &BTreeMap::new()).expect(
            "planning should succeed when an optional env-backed include variable is unset",
        );

        assert_eq!(
            plan.sources.len(),
            1,
            "only the root source should be present"
        );
    }

    #[test]
    fn reject_required_env_backed_includes_when_variable_is_unset() {
        let root = Config {
            includes: vec![Include::Environment {
                env: "PRIVATE_CONFIG".to_string(),
                optional: false,
            }],
            ..single_source_config(".", ".zshrc")
        };

        let error = derive_execution_plan_with_env(&root, &BTreeMap::new()).expect_err(
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
    fn each_included_configs_defaults_apply_only_to_its_own_sources() {
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

        let plan = derive_execution_plan_with_injectables(
            &root,
            None,
            &BTreeMap::new(),
            &|_| false,
            &|path: &Path| {
                assert_eq!(path, Path::new("/inc/a.yml"));
                Ok(included.clone())
            },
        )
        .expect("planning should succeed and isolate defaults per included config");

        assert_eq!(plan.sources.len(), 2);

        // Root's source uses copy mode with root's defaults.
        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".zshrc".to_string(),
                as_name: ".zshrc".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Copy,
                    to: "~/dest".to_string(),
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Overwrite,
                },
            })
        );

        // Included source uses symlink mode with included config's defaults.
        assert_eq!(
            plan.sources[1].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".tmux.conf".to_string(),
                as_name: ".tmux.conf".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Symlink,
                    to: "~".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            })
        );
    }

    #[test]
    fn include_bare_dollar_shorthand_is_treated_as_a_literal_path() {
        // "$" has an empty env name after stripping the "$" prefix. The
        // deserializer must treat it as a literal path, not an env-backed
        // include, to avoid silently ignoring the dollar sign.
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - .zshrc
includes:
  - $
"#;

        let config = parse_config(yaml)
            .expect("config with a bare-dollar include should parse successfully");

        assert_eq!(
            config.includes,
            vec![Include::Path {
                path: "$".to_string(),
                optional: false,
            }]
        );
    }

    #[test]
    fn parse_error_in_included_config_propagates_immediately() {
        // An include whose file returns a non-ReadConfiguration error (e.g. a
        // structural parse failure) must be propagated immediately rather than
        // collected into an IncludeNotFound set.
        let root = Config {
            includes: vec![Include::Path {
                path: "/inc/bad.yml".to_string(),
                optional: false,
            }],
            ..single_source_config(".", ".zshrc")
        };

        let error = derive_execution_plan_with_injectables(
            &root,
            None,
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
    fn source_root_bare_dollar_shorthand_is_treated_as_a_literal_path() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: $
    configs:
      - .zshrc
"#;

        let config = parse_config(yaml)
            .expect("config with a bare-dollar source root should parse successfully");

        assert_eq!(
            config.sources,
            vec![Source {
                from: SourceRoot::Path("$".to_string()),
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
            }]
        );
    }

    #[test]
    fn derive_execution_plan_uses_real_environment_and_filesystem() {
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

        let plan = derive_execution_plan(&config)
            .expect("planning should succeed when the current directory exists");

        assert_eq!(plan.repository, "~/projects/env");
        assert_eq!(plan.sources.len(), 1);
        assert_eq!(plan.sources[0].from, ".");
        assert_eq!(
            plan.sources[0].actions,
            vec![expected_file_action(".zshrc")]
        );
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should keep explicit path source roots as-is");

        assert_eq!(plan.sources.len(), 1);
        assert_eq!(plan.sources[0].from, "$PRIVATE_ENV_DIR");
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should produce overwrite-with-backup conflict policy");

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".zshrc".to_string(),
                as_name: ".zshrc".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Symlink,
                    to: "~".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::OverwriteWithBackup,
                },
            })
        );
    }

    // ---------------------------------------------------------------------------
    // Item-level option override tests
    // ---------------------------------------------------------------------------

    #[test]
    fn accept_item_level_option_overrides_in_file_config() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - file: .zshrc
        mode: copy
        to: ~/dest
        mkdir: false
        overwrite: true
        backup_on_overwrite: false
"#;

        let config =
            parse_config(yaml).expect("config should parse with item-level overrides on a file");

        let item = &config.sources[0].configs[0];
        let ConfigItem::File(file_cfg) = item else {
            panic!("expected a file config item");
        };
        assert_eq!(file_cfg.mode, Some(ActionMode::Copy));
        assert_eq!(file_cfg.to.as_deref(), Some("~/dest"));
        assert_eq!(file_cfg.mkdir, Some(false));
        assert_eq!(file_cfg.overwrite, Some(true));
        assert_eq!(file_cfg.backup_on_overwrite, Some(false));
    }

    #[test]
    fn accept_item_level_option_overrides_in_select_config() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          dotfiles: true
          mode: copy
          to: ~/dest
          mkdir: false
          overwrite: true
          backup_on_overwrite: false
"#;

        let config =
            parse_config(yaml).expect("config should parse with item-level overrides on a select");

        let item = &config.sources[0].configs[0];
        let ConfigItem::Select(sel_cfg) = item else {
            panic!("expected a select config item");
        };
        assert_eq!(sel_cfg.mode, Some(ActionMode::Copy));
        assert_eq!(sel_cfg.to.as_deref(), Some("~/dest"));
        assert_eq!(sel_cfg.mkdir, Some(false));
        assert_eq!(sel_cfg.overwrite, Some(true));
        assert_eq!(sel_cfg.backup_on_overwrite, Some(false));
    }

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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should apply item-level mode override");

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".zshrc".to_string(),
                as_name: ".zshrc".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Copy,
                    to: "~".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            })
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

        let plan = derive_execution_plan_with_env_and_fs(&config, &BTreeMap::new(), &|_| false)
            .expect("planning should apply item-level to override");

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".zshrc".to_string(),
                as_name: ".zshrc".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Symlink,
                    to: "~/item-specific".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            })
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should apply item-level overwrite override");

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".zshrc".to_string(),
                as_name: ".zshrc".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Symlink,
                    to: "~".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Overwrite,
                },
            })
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect("planning should apply select item overrides");

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::Select(PlannedSelectAction {
                dotfiles: true,
                exclude: vec![],
                options: ResolvedOptions {
                    mode: ActionMode::Copy,
                    to: "~/config".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            })
        );
    }

    #[test]
    fn cannot_set_overwrite_false_and_backup_on_overwrite_true_at_item_level() {
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

        let error = derive_execution_plan_with_env(&config, &BTreeMap::new())
            .expect_err("planning should reject explicit overwrite: false with backup_on_overwrite: true at item level");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { ref message }
                if message.contains("overwrite: false") && message.contains("backup_on_overwrite: true")
        ));
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

        let plan = derive_execution_plan_with_env(&config, &BTreeMap::new()).expect(
            "planning should succeed when backup default is inherited and overwrite is false",
        );

        assert_eq!(
            plan.sources[0].actions[0],
            PlannedAction::File(PlannedFileAction {
                file: ".zshrc".to_string(),
                as_name: ".zshrc".to_string(),
                options: ResolvedOptions {
                    mode: ActionMode::Symlink,
                    to: "~".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
                },
            })
        );
    }
}
