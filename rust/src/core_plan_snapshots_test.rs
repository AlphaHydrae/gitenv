// Core plan snapshots for the full intent + operation planning chain.
//
// These are internal module tests (not integration tests) because they use
// `IntentContext` and `derive_intent_plan_from_context`, which are `pub(crate)`
// to keep the public surface focused on the primary entrypoints
// (`derive_intent_plan`, `derive_operation_plan`).  The injectable seam tests
// (env-var expansion, directory guards, include resolution) live in the
// `intent` module's own unit tests.  This file covers the full combined
// pipeline with controlled in-memory inputs, verifying that intent and
// operation planning compose correctly end-to-end.

use crate::boundary::{ConfigReader, DirectoryProbe, EnvironmentReader};
use crate::intent::{
    ConflictPolicy, IntentAction, IntentContext, IntentFileAction, IntentPlan, IntentSelectAction,
    IntentSource, ResolvedOptions, derive_intent_plan_from_context,
};
use crate::operation::derive_operation_plan_with_injectables;
use crate::{
    ActionMode, FileOperation, LoadedConfig, OperationAction, OperationPlan, ProgramError,
    parse_config,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// --- Test doubles -----------------------------------------------------------

struct MapEnvReader(BTreeMap<String, String>);
impl EnvironmentReader for MapEnvReader {
    fn get_env_var(&self, name: &str) -> Option<String> {
        self.0.get(name).cloned()
    }
}

struct SetDirProbe(BTreeSet<String>);
impl DirectoryProbe for SetDirProbe {
    fn is_directory(&self, path: &str) -> bool {
        self.0.contains(path)
    }
}

struct AssertConfigReader {
    expected_path: PathBuf,
    loaded_config: LoadedConfig,
}
impl ConfigReader for AssertConfigReader {
    fn read_config_file(&self, path: &Path) -> Result<LoadedConfig, ProgramError> {
        assert_eq!(path, self.expected_path);
        Ok(self.loaded_config.clone())
    }
}

// --- Helper -----------------------------------------------------------------

fn derive_representative_plans(
    root_yaml: &str,
    include_yaml: &str,
) -> Result<(IntentPlan, OperationPlan), ProgramError> {
    let root = parse_config(root_yaml)?;
    let include = parse_config(include_yaml)?;
    let loaded_root = LoadedConfig {
        path: PathBuf::from("/root.yml"),
        config: root,
    };
    let include_loaded = LoadedConfig {
        path: PathBuf::from("/inc/common.yml"),
        config: include,
    };

    let env_reader = MapEnvReader(BTreeMap::from([(
        "DOTS".to_string(),
        "/repo/dots".to_string(),
    )]));
    // Guard paths are checked after home expansion; use absolute paths here.
    let dir_probe = SetDirProbe(BTreeSet::from([
        "/home/tester/shell".to_string(),
        "/feature/on".to_string(),
    ]));
    let config_reader = AssertConfigReader {
        expected_path: PathBuf::from("/inc/common.yml"),
        loaded_config: include_loaded,
    };

    let context = IntentContext {
        home_directory: PathBuf::from("/home/tester"),
        env_reader: &env_reader,
        dir_probe: &dir_probe,
        config_reader: &config_reader,
    };

    let intent_plan = derive_intent_plan_from_context(&loaded_root, &context)?;

    let operation_plan =
        derive_operation_plan_with_injectables(&intent_plan, Path::new("/home/tester"), &|path| {
            if path == Path::new("/repo/dots") {
                Ok(vec![
                    ".gitignore".to_string(),
                    ".zshrc".to_string(),
                    ".vimrc".to_string(),
                    "notes.txt".to_string(),
                ])
            } else {
                Ok(Vec::new())
            }
        })?;

    Ok((intent_plan, operation_plan))
}

fn shorthand_root_config() -> &'static str {
    r#"
version: 1
repository: "/repo"
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
includes:
  - "/inc/common.yml"
sources:
  - from: "$DOTS"
    to: "~/shell"
    when: to_exists
    configs:
      - file: ".zshrc"
        mode: copy
        to: "~/shell-private"
        mkdir: false
        overwrite: true
        backup_on_overwrite: false
      - select:
          dotfiles: true
          exclude: [".gitignore"]
          to: "dot-targets"
          mkdir: false
          overwrite: true
          backup_on_overwrite: true
  - from: "extras"
    to: "~/extras-target"
    when:
      directory_exists: "/feature/on"
    configs:
      - file: "git/config"
        as: ".gitconfig"
  - from: "ignored"
    to: "~/ignored-target"
    when:
      directory_exists: "/feature/off"
    configs:
      - ".ignoreme"
"#
}

fn canonical_root_config() -> &'static str {
    r#"
version: 1
repository: "/repo"
defaults:
  mode: symlink
  to: "~"
  mkdir: true
  overwrite: false
  backup_on_overwrite: true
includes:
  - path: "/inc/common.yml"
    optional: false
sources:
  - from:
      env: "DOTS"
      optional: false
    to: "~/shell"
    when: to_exists
    configs:
      - file: ".zshrc"
        mode: copy
        to: "~/shell-private"
        mkdir: false
        overwrite: true
        backup_on_overwrite: false
      - select:
          dotfiles: true
          exclude:
            - ".gitignore"
          to: "dot-targets"
          mkdir: false
          overwrite: true
          backup_on_overwrite: true
  - from:
      path: "extras"
    to: "~/extras-target"
    when:
      directory_exists: "/feature/on"
    configs:
      - file: "git/config"
        as: ".gitconfig"
  - from:
      path: "ignored"
    to: "~/ignored-target"
    when:
      directory_exists: "/feature/off"
    configs:
      - file: ".ignoreme"
"#
}

fn shorthand_include_config() -> &'static str {
    r#"
version: 1
repository: "/repo"
defaults:
  mode: copy
  to: "~/include-target"
  mkdir: false
  overwrite: true
  backup_on_overwrite: false
sources:
  - from: "shared"
    configs:
      - ".vimrc"
"#
}

fn canonical_include_config() -> &'static str {
    r#"
version: 1
repository: "/repo"
defaults:
  mode: copy
  to: "~/include-target"
  mkdir: false
  overwrite: true
  backup_on_overwrite: false
sources:
  - from:
      path: "shared"
    configs:
      - file: ".vimrc"
"#
}

#[test]
fn create_rich_intent_and_operation_plans() {
    let (intent_plan, operation_plan) =
        derive_representative_plans(canonical_root_config(), canonical_include_config())
            .expect("representative config should derive both plans");

    let expected_intent_plan = IntentPlan {
        repository: "/repo".to_string(),
        sources: vec![
            IntentSource {
                from: "/repo/dots".to_string(),
                actions: vec![
                    IntentAction::File(IntentFileAction {
                        file: ".zshrc".to_string(),
                        as_name: ".zshrc".to_string(),
                        options: ResolvedOptions {
                            mode: ActionMode::Copy,
                            to: "~/shell-private".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::Overwrite,
                        },
                    }),
                    IntentAction::Select(IntentSelectAction {
                        dotfiles: true,
                        exclude: vec![".gitignore".to_string()],
                        options: ResolvedOptions {
                            mode: ActionMode::Symlink,
                            to: "dot-targets".to_string(),
                            mkdir: false,
                            conflict_policy: ConflictPolicy::OverwriteWithBackup,
                        },
                    }),
                ],
            },
            IntentSource {
                from: "extras".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: "git/config".to_string(),
                    as_name: ".gitconfig".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Symlink,
                        to: "~/extras-target".to_string(),
                        mkdir: true,
                        conflict_policy: ConflictPolicy::Skip,
                    },
                })],
            },
            IntentSource {
                from: "shared".to_string(),
                actions: vec![IntentAction::File(IntentFileAction {
                    file: ".vimrc".to_string(),
                    as_name: ".vimrc".to_string(),
                    options: ResolvedOptions {
                        mode: ActionMode::Copy,
                        to: "~/include-target".to_string(),
                        mkdir: false,
                        conflict_policy: ConflictPolicy::Overwrite,
                    },
                })],
            },
        ],
    };

    let expected_operation_plan = OperationPlan {
        actions: vec![
            OperationAction::Copy(FileOperation {
                source: PathBuf::from("/repo/dots/.zshrc"),
                target: PathBuf::from("/home/tester/shell-private/.zshrc"),
                mkdir: false,
                conflict_policy: ConflictPolicy::Overwrite,
            }),
            OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/dots/.zshrc"),
                target: PathBuf::from("/home/tester/dot-targets/.zshrc"),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            }),
            OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/dots/.vimrc"),
                target: PathBuf::from("/home/tester/dot-targets/.vimrc"),
                mkdir: false,
                conflict_policy: ConflictPolicy::OverwriteWithBackup,
            }),
            OperationAction::Symlink(FileOperation {
                source: PathBuf::from("/repo/extras/git/config"),
                target: PathBuf::from("/home/tester/extras-target/.gitconfig"),
                mkdir: true,
                conflict_policy: ConflictPolicy::Skip,
            }),
            OperationAction::Copy(FileOperation {
                source: PathBuf::from("/repo/shared/.vimrc"),
                target: PathBuf::from("/home/tester/include-target/.vimrc"),
                mkdir: false,
                conflict_policy: ConflictPolicy::Overwrite,
            }),
        ],
    };

    assert_eq!(intent_plan, expected_intent_plan);
    assert_eq!(operation_plan, expected_operation_plan);
}

#[test]
fn shorthand_and_canonical_plans_are_identical() {
    let shorthand =
        derive_representative_plans(shorthand_root_config(), shorthand_include_config())
            .expect("shorthand representative config should derive both plans");
    let canonical =
        derive_representative_plans(canonical_root_config(), canonical_include_config())
            .expect("canonical representative config should derive both plans");

    assert_eq!(shorthand, canonical);
}
