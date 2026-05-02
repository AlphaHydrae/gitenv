use crate::ProgramError;
use serde::{Deserialize, Deserializer};
use std::path::Path;

/// Parsed top-level configuration model.
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

/// Global default options inherited by sources and items unless overridden.
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

/// Per-action execution mode declared in config.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionMode {
    Symlink,
    Copy,
}

/// A source directory plus the list of actions that originate from it.
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

/// Declares where a source directory path comes from.
///
/// Path variants are resolved as-is, while environment variants defer the
/// concrete path lookup to plan derivation time.
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
/// `DirectoryExists` checks an arbitrary path - useful for platform or tool detection
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
/// - `"~/path/to/file.yml"` - required literal path include.
/// - `"$VAR"` - required environment-backed path include.
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

/// A source action item, either a single file or a bulk `select` rule.
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

/// A `file` config item that targets a single source-relative file.
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

/// A `select` config item that targets a set of files based on options.
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

/// Parses YAML text into the strongly typed configuration model.
pub fn parse_config(yaml: &str) -> Result<Config, ProgramError> {
    serde_yaml::from_str(yaml).map_err(|error| ProgramError::InvalidConfiguration {
        message: format!("failed to parse config YAML: {error}"),
    })
}

/// Reads a config file from disk and parses its YAML content.
pub fn load_config(path: &Path) -> Result<Config, ProgramError> {
    let yaml = std::fs::read_to_string(path).map_err(|error| ProgramError::ReadConfiguration {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;

    parse_config(&yaml)
}

#[cfg(test)]
mod tests {
    use super::{
        ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, SelectConfig, Source,
        SourceRoot, load_config, parse_config,
    };
    use crate::ProgramError;
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

    fn unique_temp_file_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("gitenv_{prefix}_{nanos}.yml"))
    }
}
