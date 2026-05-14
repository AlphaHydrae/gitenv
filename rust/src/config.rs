use crate::{ProgramError, logging};
use log::Level;
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

/// Configuration loaded from a file path together with its parsed content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub path: PathBuf,
    pub config: Config,
}

impl Deref for LoadedConfig {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl DerefMut for LoadedConfig {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

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
    #[serde(rename = "type")]
    pub selection_type: SelectionType,
    #[serde(default)]
    pub recursive: bool,
    #[serde(default)]
    pub existing_directories_only: bool,
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

/// Selection scope for `select` config items.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelectionType {
    Dot,
    NonDot,
    All,
}

/// Parses YAML text into the strongly typed configuration model.
pub fn parse_config(yaml: &str) -> Result<Config, ProgramError> {
    logging::config(
        Level::Debug,
        "parse_config_start",
        format!("yaml_bytes={}", yaml.len()),
    );

    match serde_yaml::from_str::<Config>(yaml) {
        Ok(config) => {
            validate_config(&config)?;
            logging::config(
                Level::Info,
                "parse_config_success",
                format!(
                    "sources={} includes={}",
                    config.sources.len(),
                    config.includes.len()
                ),
            );
            Ok(config)
        }
        Err(error) => {
            logging::config(
                Level::Warn,
                "parse_config_failed",
                format!("message={error}"),
            );
            Err(ProgramError::InvalidConfiguration {
                message: format!("config YAML parse failed ({error})"),
            })
        }
    }
}

fn validate_config(config: &Config) -> Result<(), ProgramError> {
    if config.sources.is_empty() {
        return Err(ProgramError::InvalidConfiguration {
            message: "config must declare at least one source".to_string(),
        });
    }

    for source in &config.sources {
        if source.configs.is_empty() {
            return Err(ProgramError::InvalidConfiguration {
                message: format!(
                    "source `{}` must declare at least one config item",
                    source_root_label(&source.from)
                ),
            });
        }

        for item in &source.configs {
            if let ConfigItem::Select(select) = item
                && select.existing_directories_only
                && !select.recursive
            {
                return Err(ProgramError::InvalidConfiguration {
                    message: "existing_directories_only requires recursive to be true".to_string(),
                });
            }
        }
    }

    Ok(())
}

fn source_root_label(source_root: &SourceRoot) -> Cow<'_, str> {
    match source_root {
        SourceRoot::Path(path) => Cow::Borrowed(path),
        SourceRoot::ExplicitPath { path } => Cow::Borrowed(path),
        SourceRoot::Environment { env, .. } => Cow::Owned(format!("${env}")),
    }
}

/// Reads a config file from disk and parses its YAML content.
pub fn load_config(path: &Path) -> Result<LoadedConfig, ProgramError> {
    logging::config(
        Level::Debug,
        "load_config_start",
        format!("path={}", path.display()),
    );

    let yaml = std::fs::read_to_string(path).map_err(|error| {
        logging::config(
            Level::Warn,
            "load_config_failed",
            format!("path={} message={error}", path.display()),
        );
        ProgramError::ConfigurationReadFailed {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;

    logging::config(
        Level::Debug,
        "load_config_success",
        format!("path={} yaml_bytes={}", path.display(), yaml.len()),
    );

    let config = parse_config(&yaml)?;

    Ok(LoadedConfig {
        path: path.to_path_buf(),
        config,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, LoadedConfig,
        SelectConfig, SelectionType, Source, SourceRoot, load_config, parse_config,
    };
    use crate::ProgramError;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn expected_single_file_source_config(
        source_root: SourceRoot,
        source_to: Option<&str>,
        source_guard: Option<Guard>,
        file: &str,
    ) -> Config {
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: source_root,
                to: source_to.map(ToString::to_string),
                guard: source_guard,
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
        }
    }

    fn expected_config_with_includes(includes: Vec<Include>) -> Config {
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes,
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
        }
    }

    #[test]
    fn the_default_action_is_a_home_symlink() {
        assert_eq!(
            Defaults::default(),
            Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            }
        );
    }

    #[test]
    fn load_the_smallest_valid_config() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - file: .zshrc
"#;

        let mut temp_file = NamedTempFile::new().expect("temporary config file should be created");
        std::io::Write::write_all(&mut temp_file, yaml.as_bytes())
            .expect("temp config should be written");
        let loaded = load_config(temp_file.path()).expect("config should load from disk");
        let expected = expected_single_file_source_config(
            SourceRoot::Path(".".to_string()),
            None,
            None,
            ".zshrc",
        );

        assert_eq!(loaded.path, temp_file.path());
        assert_eq!(&*loaded, &expected);
    }

    #[test]
    fn allow_mutating_loaded_config_fields_without_nested_config_field_access() {
        let mut loaded = LoadedConfig {
            path: PathBuf::from("/tmp/gitenv-config.yml"),
            config: expected_single_file_source_config(
                SourceRoot::Path(".".to_string()),
                None,
                None,
                ".zshrc",
            ),
        };

        loaded.repository = "~/projects/updated".to_string();

        assert_eq!(loaded.repository, "~/projects/updated");
        assert_eq!(loaded.path, PathBuf::from("/tmp/gitenv-config.yml"));
    }

    #[test]
    fn accept_a_config_with_custom_defaults() {
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

        let config = parse_config(yaml).expect("config should parse with custom defaults");

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

        let expected = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~/.config".to_string(),
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

        assert_eq!(config, expected);
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
          type: dot
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
                    selection_type: SelectionType::Dot,
                    recursive: false,
                    existing_directories_only: false,
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
    fn normalize_omitted_and_explicit_false_recursive_select_settings() {
        let omitted_recursive_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
"#;
        let explicit_false_recursive_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          recursive: false
"#;

        let omitted_recursive = parse_config(omitted_recursive_yaml)
            .expect("config should parse when select recursive is omitted");
        let explicit_false_recursive = parse_config(explicit_false_recursive_yaml)
            .expect("config should parse when select recursive is explicitly false");

        assert_eq!(omitted_recursive, explicit_false_recursive);
    }

    #[test]
    fn parse_true_recursive_select_setting() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          recursive: true
"#;

        let config = parse_config(yaml).expect("config should parse when select recursive is true");

        let expected = Config {
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
                    exclude: vec![],
                    recursive: true,
                    existing_directories_only: false,
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
    fn parse_true_existing_directories_only_select_setting() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          recursive: true
          existing_directories_only: true
"#;

        let config = parse_config(yaml)
            .expect("config should parse when existing_directories_only is true with recursive");

        let expected = Config {
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
                    existing_directories_only: true,
                    exclude: vec![],
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
    fn reject_existing_directories_only_when_recursive_is_not_enabled() {
        let omitted_recursive_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          existing_directories_only: true
"#;
        let explicit_false_recursive_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          recursive: false
          existing_directories_only: true
"#;

        let error_omitted = parse_config(omitted_recursive_yaml)
            .expect_err("existing_directories_only should be rejected without recursive");
        let error_explicit_false = parse_config(explicit_false_recursive_yaml)
            .expect_err("existing_directories_only should be rejected when recursive is false");

        assert_eq!(
            error_omitted,
            ProgramError::InvalidConfiguration {
                message: "existing_directories_only requires recursive to be true".to_string(),
            }
        );
        assert_eq!(error_omitted, error_explicit_false);
    }

    #[test]
    fn normalize_omitted_and_explicit_false_existing_directories_only_select_settings() {
        let omitted_flag_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          recursive: true
"#;
        let explicit_false_flag_yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
          recursive: true
          existing_directories_only: false
"#;

        let omitted_flag = parse_config(omitted_flag_yaml)
            .expect("config should parse when existing_directories_only is omitted");
        let explicit_false_flag = parse_config(explicit_false_flag_yaml)
            .expect("config should parse when existing_directories_only is explicitly false");

        assert_eq!(omitted_flag, explicit_false_flag);
    }

    #[test]
    fn accept_each_selection_type_value_in_select_configs() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs:
      - select:
          type: dot
      - select:
          type: non-dot
      - select:
          type: all
          exclude:
            - .DS_Store
"#;

        let config = parse_config(yaml)
            .expect("config should parse with dot, non-dot, and all selection types");

        let expected = Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![],
            sources: vec![Source {
                from: SourceRoot::Path(".".to_string()),
                to: None,
                guard: None,
                configs: vec![
                    ConfigItem::Select(SelectConfig {
                        selection_type: SelectionType::Dot,
                        exclude: vec![],
                        recursive: false,
                        existing_directories_only: false,
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                    ConfigItem::Select(SelectConfig {
                        selection_type: SelectionType::NonDot,
                        exclude: vec![],
                        recursive: false,
                        existing_directories_only: false,
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                    ConfigItem::Select(SelectConfig {
                        selection_type: SelectionType::All,
                        exclude: vec![".DS_Store".to_string()],
                        recursive: false,
                        existing_directories_only: false,
                        mode: None,
                        to: None,
                        mkdir: None,
                        overwrite: None,
                        backup_on_overwrite: None,
                    }),
                ],
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
          type: dot
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
          type: dot
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
        // Create a temp path that is guaranteed not to exist at call time.
        let temp_file = NamedTempFile::new().expect("temp path should be created");
        let file_path = temp_file.path().to_path_buf();
        drop(temp_file);

        let error = load_config(&file_path).expect_err("missing file should fail to load");

        assert!(matches!(
            &error,
            ProgramError::ConfigurationReadFailed { path, message }
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
    fn reject_a_config_without_sources() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources: []
"#;

        let error = parse_config(yaml).expect_err("config should fail without sources");

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "config must declare at least one source".to_string(),
            }
        );
    }

    #[test]
    fn reject_a_source_without_configs() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from: "."
    configs: []
"#;

        let error = parse_config(yaml).expect_err("source should fail without config items");

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "source `.` must declare at least one config item".to_string(),
            }
        );
    }

    #[test]
    fn reject_an_explicit_path_source_without_configs() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from:
      path: /tmp/private
    configs: []
"#;

        let error = parse_config(yaml).expect_err("source should fail without config items");

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "source `/tmp/private` must declare at least one config item".to_string(),
            }
        );
    }

    #[test]
    fn reject_an_environment_backed_source_without_configs() {
        let yaml = r#"
version: 1
repository: ~/projects/env
sources:
  - from:
      env: PRIVATE_ENV_DIR
    configs: []
"#;

        let error = parse_config(yaml).expect_err("source should fail without config items");

        assert_eq!(
            error,
            ProgramError::InvalidConfiguration {
                message: "source `$PRIVATE_ENV_DIR` must declare at least one config item"
                    .to_string(),
            }
        );
    }

    #[test]
    fn reject_a_malformed_config() {
        let yaml = "version: [";

        let error = parse_config(yaml).expect_err("malformed yaml should fail");

        assert!(matches!(
            error,
            ProgramError::InvalidConfiguration { message }
                if message.contains("config YAML parse failed")
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
            config,
            expected_single_file_source_config(
                SourceRoot::Path("$".to_string()),
                None,
                None,
                ".zshrc",
            )
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

        assert_eq!(
            config,
            expected_single_file_source_config(
                SourceRoot::Path("vscode".to_string()),
                Some("~/Library/Application Support/Code/User"),
                Some(Guard::ToExists),
                "keybindings.json",
            )
        );
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

        assert_eq!(
            config,
            expected_single_file_source_config(
                SourceRoot::Path("macos".to_string()),
                None,
                Some(Guard::DirectoryExists("/Applications".to_string())),
                ".macos-defaults",
            )
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
            config,
            expected_single_file_source_config(
                SourceRoot::Path("vscode".to_string()),
                Some("~/Library/Application Support/Code/User"),
                None,
                "settings.json",
            )
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
            config,
            expected_config_with_includes(vec![Include::Path {
                path: "~/projects/private-env/.gitenv.yml".to_string(),
                optional: false,
            }])
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
            config,
            expected_config_with_includes(vec![Include::Environment {
                env: "WORK_ENV_CONFIG".to_string(),
                optional: false,
            }])
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
            config,
            expected_config_with_includes(vec![Include::Path {
                path: "~/projects/work-env/.gitenv.yml".to_string(),
                optional: true,
            }])
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
            config,
            expected_config_with_includes(vec![Include::Environment {
                env: "EXTRA_ENV_CONFIG".to_string(),
                optional: true,
            }])
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

        assert_eq!(
            config,
            expected_single_file_source_config(
                SourceRoot::Path(".".to_string()),
                None,
                None,
                ".zshrc"
            )
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
            config,
            expected_config_with_includes(vec![Include::Path {
                path: "$".to_string(),
                optional: false,
            }])
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
                    mode: Some(ActionMode::Copy),
                    to: Some("~/dest".to_string()),
                    mkdir: Some(false),
                    overwrite: Some(true),
                    backup_on_overwrite: Some(false),
                })],
            }],
        };

        assert_eq!(config, expected);
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
              type: dot
              mode: copy
              to: ~/dest
              mkdir: false
              overwrite: true
              backup_on_overwrite: false
"#;

        let config =
            parse_config(yaml).expect("config should parse with item-level overrides on a select");

        let expected = Config {
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
                    exclude: vec![],
                    recursive: false,
                    existing_directories_only: false,
                    mode: Some(ActionMode::Copy),
                    to: Some("~/dest".to_string()),
                    mkdir: Some(false),
                    overwrite: Some(true),
                    backup_on_overwrite: Some(false),
                })],
            }],
        };

        assert_eq!(config, expected);
    }
}
