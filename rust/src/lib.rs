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
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectConfig {
    pub dotfiles: bool,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    InvalidConfiguration { message: String },
    ReadConfiguration { path: PathBuf, message: String },
    MissingEnvironment { vars: Vec<String> },
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
    pub mode: ActionMode,
    pub to: String,
    pub mkdir: bool,
    pub conflict_policy: ConflictPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSelectAction {
    pub dotfiles: bool,
    pub exclude: Vec<String>,
    pub mode: ActionMode,
    pub to: String,
    pub mkdir: bool,
    pub conflict_policy: ConflictPolicy,
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

/// Core planning function. `is_directory` is injected so tests can evaluate guards
/// against a controlled set of paths rather than the real filesystem.
pub fn derive_execution_plan_with_env_and_fs(
    config: &Config,
    environment: &BTreeMap<String, String>,
    is_directory: &impl Fn(&str) -> bool,
) -> Result<ExecutionPlan, ProgramError> {
    let mut missing_environment = BTreeSet::new();
    let mut sources = Vec::new();
    let conflict_policy = resolve_conflict_policy(
        config.defaults.overwrite,
        config.defaults.backup_on_overwrite,
    );

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
                    missing_environment.insert(env.clone());
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

        sources.push(PlannedSource {
            from,
            actions: source
                .configs
                .iter()
                .map(|item| match item {
                    ConfigItem::File(file_config) => PlannedAction::File(PlannedFileAction {
                        file: file_config.file.clone(),
                        as_name: file_config
                            .as_name
                            .clone()
                            .unwrap_or_else(|| file_config.file.clone()),
                        mode: config.defaults.mode.clone(),
                        to: source_to.clone(),
                        mkdir: config.defaults.mkdir,
                        conflict_policy: conflict_policy.clone(),
                    }),
                    ConfigItem::Select(select_config) => {
                        PlannedAction::Select(PlannedSelectAction {
                            dotfiles: select_config.dotfiles,
                            exclude: select_config.exclude.clone(),
                            mode: config.defaults.mode.clone(),
                            to: source_to.clone(),
                            mkdir: config.defaults.mkdir,
                            conflict_policy: conflict_policy.clone(),
                        })
                    }
                })
                .collect(),
        });
    }

    if !missing_environment.is_empty() {
        return Err(ProgramError::MissingEnvironment {
            vars: missing_environment.into_iter().collect(),
        });
    }

    Ok(ExecutionPlan {
        repository: config.repository.clone(),
        sources,
    })
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

pub fn run() -> Result<ProgramOutput, ProgramError> {
    Ok(ProgramOutput {
        message: "Hello, World!",
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ActionMode, Config, ConfigItem, ConflictPolicy, Defaults, ExecutionPlan, FileConfig, Guard,
        PlannedAction, PlannedFileAction, PlannedSelectAction, PlannedSource, ProgramError,
        SelectConfig, Source, SourceRoot, derive_execution_plan_with_env,
        derive_execution_plan_with_env_and_fs, load_config, parse_config, run,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;
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
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![
                    ConfigItem::File(FileConfig {
                        file: ".zshrc".to_string(),
                        as_name: None,
                    }),
                    ConfigItem::Select(SelectConfig {
                        dotfiles: true,
                        exclude: vec![".DS_Store".to_string()],
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
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
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
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::Select(SelectConfig {
                    dotfiles: true,
                    exclude: vec![".DS_Store".to_string(), ".git".to_string()],
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
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
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
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![
                    ConfigItem::File(FileConfig {
                        file: ".zshrc".to_string(),
                        as_name: None,
                    }),
                    ConfigItem::File(FileConfig {
                        file: ".tmux".to_string(),
                        as_name: Some(".tmux.conf".to_string()),
                    }),
                    ConfigItem::Select(SelectConfig {
                        dotfiles: true,
                        exclude: vec![".git".to_string()],
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
                        mode: ActionMode::Copy,
                        to: "~/dest".to_string(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Overwrite,
                    }),
                    PlannedAction::File(PlannedFileAction {
                        file: ".tmux".to_string(),
                        as_name: ".tmux.conf".to_string(),
                        mode: ActionMode::Copy,
                        to: "~/dest".to_string(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Overwrite,
                    }),
                    PlannedAction::Select(PlannedSelectAction {
                        dotfiles: true,
                        exclude: vec![".git".to_string()],
                        mode: ActionMode::Copy,
                        to: "~/dest".to_string(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Overwrite,
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
            sources: vec![Source {
                from: SourceRoot::ExplicitPath {
                    path: "$PRIVATE_ENV_DIR".to_string(),
                },
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
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
                    mode: ActionMode::Copy,
                    to: "~/.config".to_string(),
                    mkdir: false,
                    conflict_policy: ConflictPolicy::Overwrite,
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
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".zshrc".to_string(),
                    as_name: None,
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
                    mode: ActionMode::Symlink,
                    to: "~".to_string(),
                    mkdir: true,
                    conflict_policy: ConflictPolicy::Skip,
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
            sources: vec![Source {
                from: SourceRoot::Path("vscode".to_string()),
                to: Some("~/Library/Application Support/Code/User".to_string()),
                guard: None,
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
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
                mode: ActionMode::Symlink,
                to: "~/Library/Application Support/Code/User".to_string(),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            })
        );
    }

    #[test]
    fn include_a_source_when_to_exists_guard_is_satisfied() {
        let config = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![Source {
                from: SourceRoot::Path("vscode".to_string()),
                to: Some("~/Library/Application Support/Code/User".to_string()),
                guard: Some(Guard::ToExists),
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
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
            sources: vec![Source {
                from: SourceRoot::Path("vscode".to_string()),
                to: Some("~/Library/Application Support/Code/User".to_string()),
                guard: Some(Guard::ToExists),
                configs: vec![ConfigItem::File(FileConfig {
                    file: "settings.json".to_string(),
                    as_name: None,
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
            sources: vec![Source {
                from: SourceRoot::Path("macos".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists("/Applications".to_string())),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".macos-defaults".to_string(),
                    as_name: None,
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
            sources: vec![Source {
                from: SourceRoot::Path("macos".to_string()),
                to: None,
                guard: Some(Guard::DirectoryExists("/Applications".to_string())),
                configs: vec![ConfigItem::File(FileConfig {
                    file: ".macos-defaults".to_string(),
                    as_name: None,
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
}
