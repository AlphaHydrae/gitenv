use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
                        to: config.defaults.to.clone(),
                        mkdir: config.defaults.mkdir,
                        conflict_policy: conflict_policy.clone(),
                    }),
                    ConfigItem::Select(select_config) => {
                        PlannedAction::Select(PlannedSelectAction {
                            dotfiles: select_config.dotfiles,
                            exclude: select_config.exclude.clone(),
                            mode: config.defaults.mode.clone(),
                            to: config.defaults.to.clone(),
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
        ActionMode, Config, ConfigItem, ConflictPolicy, Defaults, ExecutionPlan, FileConfig,
        PlannedAction, PlannedFileAction, PlannedSelectAction, PlannedSource, ProgramError,
        SelectConfig, Source, SourceRoot, derive_execution_plan_with_env, load_config,
        parse_config, run,
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

    fn unique_temp_file_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("gitenv_{prefix}_{nanos}.yml"))
    }
}
