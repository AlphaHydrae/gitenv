use gitenv::{
    ActionMode, Config, ConfigItem, Defaults, FileConfig, Guard, Include, SelectConfig,
    SelectionType, Source, SourceRoot, parse_config,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReadmeExample {
    id: String,
    yaml: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExtractError {
    MissingFenceLanguage {
        line: usize,
    },
    MissingIdForYamlBlock {
        line: usize,
    },
    IdNotFollowedByYamlBlock {
        id: String,
        line: usize,
    },
    DuplicateId {
        id: String,
        first_line: usize,
        duplicate_line: usize,
    },
    UnclosedFence {
        line: usize,
    },
}

fn parse_readme_config_id(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let prefix = "<!-- readme-config-id:";
    let suffix = "-->";
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(suffix) {
        return None;
    }

    let body = trimmed
        .trim_start_matches(prefix)
        .trim_end_matches(suffix)
        .trim();
    if body.is_empty() {
        return None;
    }

    Some(body.to_string())
}

// Extract README YAML examples with strict ID and fence-language validation.
fn extract_readme_yaml_examples(markdown: &str) -> Result<Vec<ReadmeExample>, ExtractError> {
    let mut examples = Vec::new();
    let mut seen_ids = BTreeMap::<String, usize>::new();
    let mut pending_id: Option<(String, usize)> = None;

    let mut inside_fence = false;
    let mut fence_language = String::new();
    let mut fence_start_line = 0usize;
    let mut fence_yaml_id = String::new();
    let mut fence_body: Vec<String> = Vec::new();

    for (index, line) in markdown.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = line.trim();

        if !inside_fence {
            if let Some(id) = parse_readme_config_id(line) {
                if let Some((pending, pending_line)) = pending_id.take() {
                    return Err(ExtractError::IdNotFollowedByYamlBlock {
                        id: pending,
                        line: pending_line,
                    });
                }

                if let Some(first_line) = seen_ids.get(&id) {
                    return Err(ExtractError::DuplicateId {
                        id,
                        first_line: *first_line,
                        duplicate_line: line_number,
                    });
                }

                pending_id = Some((id, line_number));
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("```") {
                let info = rest.trim();
                if info.is_empty() {
                    return Err(ExtractError::MissingFenceLanguage { line: line_number });
                }

                let language = info
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();

                if language == "yaml" {
                    let (id, id_line) = pending_id
                        .take()
                        .ok_or(ExtractError::MissingIdForYamlBlock { line: line_number })?;
                    seen_ids.insert(id.clone(), id_line);
                    fence_yaml_id = id;
                } else if let Some((id, id_line)) = pending_id.take() {
                    return Err(ExtractError::IdNotFollowedByYamlBlock { id, line: id_line });
                }

                inside_fence = true;
                fence_language = language;
                fence_start_line = line_number;
                fence_body.clear();
                continue;
            }

            if !trimmed.is_empty()
                && let Some((id, id_line)) = pending_id.take()
            {
                return Err(ExtractError::IdNotFollowedByYamlBlock { id, line: id_line });
            }

            continue;
        }

        if trimmed == "```" {
            if fence_language == "yaml" {
                examples.push(ReadmeExample {
                    id: fence_yaml_id.clone(),
                    yaml: fence_body.join("\n"),
                });
            }

            inside_fence = false;
            fence_language.clear();
            fence_yaml_id.clear();
            fence_body.clear();
            continue;
        }

        fence_body.push(line.to_string());
    }

    if inside_fence {
        return Err(ExtractError::UnclosedFence {
            line: fence_start_line,
        });
    }

    if let Some((id, line)) = pending_id {
        return Err(ExtractError::IdNotFollowedByYamlBlock { id, line });
    }

    Ok(examples)
}

fn readme_path() -> String {
    format!("{}/README.md", env!("CARGO_MANIFEST_DIR"))
}

fn readme_text() -> String {
    fs::read_to_string(readme_path()).expect("rust README should be readable")
}

fn source(from: &str, to: Option<&str>, configs: Vec<ConfigItem>) -> Source {
    source_with_root(SourceRoot::Path(from.to_string()), to, None, configs)
}

fn source_with_root(
    from: SourceRoot,
    to: Option<&str>,
    guard: Option<Guard>,
    configs: Vec<ConfigItem>,
) -> Source {
    Source {
        from,
        to: to.map(ToString::to_string),
        guard,
        configs,
    }
}

fn shorthand_file_item(file: &str) -> ConfigItem {
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

fn file_item(
    file: &str,
    as_name: Option<&str>,
    mode: Option<ActionMode>,
    to: Option<&str>,
    mkdir: Option<bool>,
    overwrite: Option<bool>,
    backup_on_overwrite: Option<bool>,
) -> ConfigItem {
    ConfigItem::File(FileConfig {
        file: file.to_string(),
        as_name: as_name.map(ToString::to_string),
        mode,
        to: to.map(ToString::to_string),
        mkdir,
        overwrite,
        backup_on_overwrite,
    })
}

fn select_item(
    selection_type: SelectionType,
    include: &[&str],
    exclude: &[&str],
    mode: Option<ActionMode>,
) -> ConfigItem {
    select_item_with_options(
        selection_type,
        false,
        false,
        include,
        exclude,
        mode,
        None,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_item_with_options(
    selection_type: SelectionType,
    recursive: bool,
    existing_directories_only: bool,
    include: &[&str],
    exclude: &[&str],
    mode: Option<ActionMode>,
    to: Option<&str>,
    mkdir: Option<bool>,
    overwrite: Option<bool>,
    backup_on_overwrite: Option<bool>,
) -> ConfigItem {
    ConfigItem::Select(SelectConfig {
        selection_type,
        recursive,
        existing_directories_only,
        include: include.iter().map(ToString::to_string).collect(),
        exclude: exclude.iter().map(ToString::to_string).collect(),
        mode,
        to: to.map(ToString::to_string),
        mkdir,
        overwrite,
        backup_on_overwrite,
    })
}

fn expected_readme_configs() -> BTreeMap<String, Config> {
    let mut expected = BTreeMap::new();

    expected.insert(
        "minimal".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            sources: vec![source(".", None, vec![shorthand_file_item(".zshrc")])],
            includes: vec![],
        },
    );

    expected.insert(
        "repository".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(".", None, vec![shorthand_file_item(".zshrc")])],
            includes: vec![],
        },
    );

    expected.insert(
        "defaults".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: false,
            },
            sources: vec![source(".", None, vec![shorthand_file_item(".zshrc")])],
            includes: vec![],
        },
    );

    expected.insert(
        "rename-targets".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                ".",
                None,
                vec![
                    shorthand_file_item("zshrc.local"),
                    file_item(
                        "zshrc.shared",
                        Some(".zshrc.local"),
                        None,
                        None,
                        None,
                        None,
                        None,
                    ),
                ],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "sub-folders".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![
                source("zsh", None, vec![shorthand_file_item(".zshrc")]),
                source(
                    "editor",
                    None,
                    vec![
                        shorthand_file_item("settings.json"),
                        shorthand_file_item("keybindings.json"),
                    ],
                ),
            ],
            includes: vec![],
        },
    );

    expected.insert(
        "destination".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                ".",
                Some(".config/gitenv-demo"),
                vec![
                    shorthand_file_item(".zshrc"),
                    shorthand_file_item(".gitconfig"),
                ],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "copy-files".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                ".",
                None,
                vec![
                    file_item(
                        ".gitconfig",
                        None,
                        Some(ActionMode::Copy),
                        None,
                        None,
                        None,
                        None,
                    ),
                    file_item(
                        ".config/tool.json",
                        None,
                        Some(ActionMode::Copy),
                        Some(".config/tool"),
                        None,
                        None,
                        None,
                    ),
                ],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "file-item-mkdir".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                ".",
                None,
                vec![
                    file_item(
                        ".config/tool/config.toml",
                        None,
                        None,
                        Some(".config/tool"),
                        Some(true),
                        None,
                        None,
                    ),
                    shorthand_file_item(".zshrc"),
                ],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "environment-backed-source-roots".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![
                source_with_root(
                    SourceRoot::Environment {
                        env: "DOTFILES_ROOT".to_string(),
                        optional: false,
                    },
                    None,
                    None,
                    vec![shorthand_file_item(".zshrc")],
                ),
                source_with_root(
                    SourceRoot::Environment {
                        env: "TOOL_CONFIG_ROOT".to_string(),
                        optional: true,
                    },
                    Some(".config/tool"),
                    None,
                    vec![shorthand_file_item("config.toml")],
                ),
            ],
            includes: vec![],
        },
    );

    expected.insert(
        "source-level-guards".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![
                source_with_root(
                    SourceRoot::Path(".".to_string()),
                    Some(".config/gitenv-demo"),
                    Some(Guard::ToExists),
                    vec![shorthand_file_item(".zshrc")],
                ),
                source_with_root(
                    SourceRoot::Path("macos".to_string()),
                    None,
                    Some(Guard::DirectoryExists("/Applications".to_string())),
                    vec![shorthand_file_item("karabiner.json")],
                ),
            ],
            includes: vec![],
        },
    );

    expected.insert(
        "select-multiple-files".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                "profiles",
                Some(".local/share/profiles"),
                vec![select_item(
                    SelectionType::Dot,
                    &["dotfiles/**"],
                    &["**/*.tmp", "private/**"],
                    None,
                )],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "recursive-select-existing-directories-only".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                "profiles",
                Some(".local/share/profiles"),
                vec![select_item_with_options(
                    SelectionType::All,
                    true,
                    true,
                    &["**/*.profile"],
                    &[],
                    None,
                    None,
                    None,
                    None,
                    None,
                )],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "select-item-overrides".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                "profiles",
                None,
                vec![select_item_with_options(
                    SelectionType::NonDot,
                    false,
                    false,
                    &["**/*.json"],
                    &[],
                    Some(ActionMode::Copy),
                    Some(".config/profiles"),
                    Some(true),
                    Some(true),
                    Some(true),
                )],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "overwrite-and-backup".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            sources: vec![source(
                ".",
                None,
                vec![
                    file_item(".zshrc", None, None, None, None, Some(true), None),
                    file_item(
                        ".gitconfig",
                        None,
                        Some(ActionMode::Copy),
                        None,
                        None,
                        Some(true),
                        Some(true),
                    ),
                ],
            )],
            includes: vec![],
        },
    );

    expected.insert(
        "composition-with-includes".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults {
                mode: ActionMode::Symlink,
                to: "~".to_string(),
                mkdir: true,
                overwrite: false,
                backup_on_overwrite: true,
            },
            includes: vec![
                Include::Path {
                    path: "shared-config.yml".to_string(),
                    optional: false,
                },
                Include::Path {
                    path: "~/.gitenv-private.yml".to_string(),
                    optional: false,
                },
            ],
            sources: vec![source(".", None, vec![shorthand_file_item(".zshrc")])],
        },
    );

    expected.insert(
        "optional-includes".to_string(),
        Config {
            version: 1,
            repository: "~/projects/env".to_string(),
            defaults: Defaults::default(),
            includes: vec![
                Include::Path {
                    path: "shared-config.yml".to_string(),
                    optional: false,
                },
                Include::Path {
                    path: "~/.local/gitenv-private.yml".to_string(),
                    optional: true,
                },
                Include::Environment {
                    env: "CUSTOM_GITENV_CONFIG".to_string(),
                    optional: true,
                },
            ],
            sources: vec![source(".", None, vec![shorthand_file_item(".zshrc")])],
        },
    );

    expected
}

#[test]
fn validate_readme_examples_against_declared_expectations() {
    let readme = readme_text();
    let examples = extract_readme_yaml_examples(&readme).unwrap_or_else(|error| {
        panic!(
            "README examples must follow ID and code fence language rules: {:?}. \
             If README examples changed intentionally, ensure every fenced block \
             has a language, each YAML block has a unique readme-config-id marker, \
             and update expected_readme_configs() in tests/readme_examples.rs.",
            error
        )
    });

    let expected = expected_readme_configs();
    let expected_ids = expected.keys().cloned().collect::<BTreeSet<_>>();

    let extracted_ids = examples
        .iter()
        .map(|example| example.id.clone())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        extracted_ids, expected_ids,
        "README example IDs must match expected IDs exactly. If README YAML \
         examples changed intentionally, update expected_readme_configs() in \
         tests/readme_examples.rs and keep readme-config-id markers unique."
    );

    assert_eq!(
        examples.len(),
        expected.len(),
        "README YAML block count must match expected example count. If README \
         examples changed intentionally, update expected_readme_configs() in \
         tests/readme_examples.rs."
    );

    let mut parsed_examples = BTreeMap::<String, Config>::new();
    for example in examples {
        let parsed = parse_config(&example.yaml).unwrap_or_else(|error| {
            panic!(
                "README example '{}' must parse as a valid config: {}. If this \
                 example was edited intentionally, fix the README YAML and then \
                 update expected_readme_configs() in tests/readme_examples.rs \
                 if the parsed shape changed.",
                example.id, error
            )
        });
        parsed_examples.insert(example.id, parsed);
    }

    assert_eq!(
        parsed_examples, expected,
        "README parsed configs must match expected configs exactly. If README \
         examples changed intentionally, update expected_readme_configs() in \
         tests/readme_examples.rs to keep the test in sync."
    );
}

#[test]
fn extract_examples_when_ids_and_languages_are_valid() {
    let markdown = r#"
<!-- readme-config-id: one -->
```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - .zshrc
```

```text
status output
```

<!-- readme-config-id: two -->
```yaml
version: 1
repository: "~/projects/env"
sources:
  - from: "."
    configs:
      - .gitconfig
```
"#;

    let examples = extract_readme_yaml_examples(markdown).expect("markdown should parse");
    assert_eq!(
        examples,
        vec![
            ReadmeExample {
                id: "one".to_string(),
                yaml: concat!(
                    "version: 1\n",
                    "repository: \"~/projects/env\"\n",
                    "sources:\n",
                    "  - from: \".\"\n",
                    "    configs:\n",
                    "      - .zshrc"
                )
                .to_string(),
            },
            ReadmeExample {
                id: "two".to_string(),
                yaml: concat!(
                    "version: 1\n",
                    "repository: \"~/projects/env\"\n",
                    "sources:\n",
                    "  - from: \".\"\n",
                    "    configs:\n",
                    "      - .gitconfig"
                )
                .to_string(),
            },
        ]
    );
}

#[test]
fn cannot_extract_examples_when_a_code_fence_language_is_missing() {
    let markdown = "```\nno language\n```\n";
    assert_eq!(
        extract_readme_yaml_examples(markdown),
        Err(ExtractError::MissingFenceLanguage { line: 1 })
    );
}

#[test]
fn cannot_extract_examples_when_a_yaml_block_has_no_id() {
    let markdown = "```yaml\nversion: 1\n```\n";
    assert_eq!(
        extract_readme_yaml_examples(markdown),
        Err(ExtractError::MissingIdForYamlBlock { line: 1 })
    );
}

#[test]
fn cannot_extract_examples_when_an_id_has_no_following_yaml_block() {
    let markdown = "<!-- readme-config-id: orphan -->\nSome prose\n";
    assert_eq!(
        extract_readme_yaml_examples(markdown),
        Err(ExtractError::IdNotFollowedByYamlBlock {
            id: "orphan".to_string(),
            line: 1,
        })
    );
}

#[test]
fn cannot_extract_examples_when_an_id_precedes_a_non_yaml_block() {
    let markdown = "<!-- readme-config-id: orphan -->\n```text\nhello\n```\n";
    assert_eq!(
        extract_readme_yaml_examples(markdown),
        Err(ExtractError::IdNotFollowedByYamlBlock {
            id: "orphan".to_string(),
            line: 1,
        })
    );
}

#[test]
fn cannot_extract_examples_when_an_id_is_reused() {
    let markdown = concat!(
        "<!-- readme-config-id: duplicate -->\n",
        "```yaml\nversion: 1\nrepository: \"~/projects/env\"\n",
        "sources:\n  - from: \".\"\n    configs:\n      - .zshrc\n```\n",
        "<!-- readme-config-id: duplicate -->\n",
        "```yaml\nversion: 1\nrepository: \"~/projects/env\"\n",
        "sources:\n  - from: \".\"\n    configs:\n      - .gitconfig\n```\n"
    );

    assert_eq!(
        extract_readme_yaml_examples(markdown),
        Err(ExtractError::DuplicateId {
            id: "duplicate".to_string(),
            first_line: 1,
            duplicate_line: 10,
        })
    );
}

#[test]
fn cannot_extract_examples_when_a_fence_is_not_closed() {
    let markdown = "<!-- readme-config-id: one -->\n```yaml\nversion: 1\n";
    assert_eq!(
        extract_readme_yaml_examples(markdown),
        Err(ExtractError::UnclosedFence { line: 2 })
    );
}

// Scenario tests: validate that README examples execute correctly in temporary
// directories. These tests verify that each documented configuration can be
// parsed, loaded, and executed via the CLI.

fn create_gitenv_command_for_home(home: &TempDir, repo: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gitenv"));
    command
        .env("HOME", home.path())
        .env("GITENV_REPO", repo.path())
        // Keep tests deterministic when CI sets global config path variables.
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GITENV_CONFIG");
    command
}

fn write_readme_example_config(home: &TempDir, yaml: &str) {
    let config_path = home
        .path()
        .join(".config")
        .join("gitenv")
        .join("config.yml");
    fs::create_dir_all(
        config_path
            .parent()
            .expect("config directory should have a parent"),
    )
    .expect("config directory should be created");
    fs::write(config_path, yaml).expect("config file should be written");
}

fn create_stub_includes(home: &TempDir, config: &Config) {
    // For each include in the config, create a stub file if it's a path-based
    // include. This ensures that `gitenv info` can load the config without
    // failing on missing include files. Stub includes must have at least one
    // valid source (even if the source path doesn't exist).
    for include in &config.includes {
        match include {
            Include::Path { path, .. } => {
                let include_path = if path.starts_with('~') {
                    home.path().join(path.trim_start_matches("~/"))
                } else {
                    // For relative paths, resolve relative to the config directory
                    home.path().join(".config").join("gitenv").join(path)
                };

                // Create parent directories if needed
                if let Some(parent) = include_path.parent() {
                    fs::create_dir_all(parent).ok();
                }

                // Write a minimal stub config with one source (even if path doesn't exist).
                // This allows includes to parse successfully during info/apply.
                let stub_config = concat!(
                    "version: 1\n",
                    "repository: \"~/tmp\"\n",
                    "sources:\n",
                    "  - from: \".\"\n",
                    "    configs:\n",
                    "      - stub.txt\n"
                );
                fs::write(include_path, stub_config).ok();
            }
            Include::Environment { .. } => {
                // Environment-backed includes are resolved at runtime; no file creation needed
            }
        }
    }
}

fn set_source_root_environment(command: &mut Command, repo: &TempDir, config: &Config) {
    for source in &config.sources {
        if let SourceRoot::Environment { env, .. } = &source.from {
            let source_dir = repo.path().join(format!("source-{env}"));
            fs::create_dir_all(&source_dir).expect("environment source directory should exist");
            command.env(env, source_dir);
        }
    }
}

#[test]
fn readme_examples_execute_without_error_in_temporary_directories() {
    let readme = readme_text();
    let examples = extract_readme_yaml_examples(&readme).expect("README examples must be valid");

    // Verify that each README example can be executed via the CLI without error.
    for example in examples {
        // Parse the example to get the Config structure so we can create stub
        // include files.
        let config = match parse_config(&example.yaml) {
            Ok(config) => config,
            Err(error) => {
                panic!(
                    "README example '{}' should parse as valid config, but failed: {}",
                    example.id, error
                );
            }
        };

        // Create temporary home and repository directories.
        let home = TempDir::new().unwrap_or_else(|_| {
            panic!(
                "temporary home directory for '{}' should be created",
                example.id
            )
        });
        let repo = TempDir::new().unwrap_or_else(|_| {
            panic!(
                "temporary repository for '{}' should be created",
                example.id
            )
        });

        // Write the example config to the temporary home.
        write_readme_example_config(&home, &example.yaml);

        // Create stub files for any includes referenced in the config.
        create_stub_includes(&home, &config);

        // Run `gitenv info` with the example config and verify it succeeds.
        let mut command = create_gitenv_command_for_home(&home, &repo);
        set_source_root_environment(&mut command, &repo, &config);

        let output = command.output().unwrap_or_else(|error| {
            panic!(
                "gitenv info should execute for example '{}': {}",
                example.id, error
            )
        });

        assert!(
            output.status.success(),
            "gitenv info should succeed for example '{}', but exited with code {:?}. \
             stderr: {}",
            example.id,
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
