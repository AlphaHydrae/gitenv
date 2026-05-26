// Core plan snapshots for the full intent + operation planning chain.
//
// These are internal module tests (not integration tests) because they use
// `IntentContext` and `derive_intent_plan`, which are `pub(crate)`
// to keep the public surface focused on the primary entrypoints
// (`derive_intent_plan`, `derive_operation_plan`).  The injectable seam tests
// (env-var expansion, directory guards, include resolution) live in the
// `intent` module's own unit tests.  This file covers the full combined
// pipeline with controlled in-memory inputs, verifying that intent and
// operation planning compose correctly end-to-end.

use crate::boundary::{
    ConfigReader, DirectoryProbe, EnvironmentReader, SourcePathRequirement, SourceReadError,
    test_doubles::FnSourceAvailabilityReader,
};
use crate::intent::{
    ConflictPolicy, IntentAction, IntentContext, IntentFileAction, IntentPlan, IntentSelectAction,
    IntentSource, ResolvedOptions, derive_intent_plan, validated_test_globs,
};
use crate::operation::{
    OperationContext, OperationEntry, PlannedOperationAction, derive_operation_plan,
};
use crate::{
    ActionMode, FileOperation, LoadedConfig, OperationAction, OperationPlan, ProgramError,
    SelectionType, SourceAvailability, parse_config,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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

fn readable_source_path(
    _path: &Path,
    _requirement: SourcePathRequirement,
) -> Result<(), SourceReadError> {
    Ok(())
}

fn available_operation_plan(actions: Vec<OperationAction>) -> OperationPlan {
    OperationPlan {
        entries: actions
            .into_iter()
            .map(|action| {
                OperationEntry::Action(PlannedOperationAction {
                    action,
                    source_availability: SourceAvailability::Available,
                    skip_reason: None,
                })
            })
            .collect(),
    }
}

// --- Helper -----------------------------------------------------------------

fn derive_representative_plans(
    root_yaml: &str,
    include_yaml: &str,
    dots_root: &Path,
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

    seed_dots_repository(dots_root);

    let env_reader = MapEnvReader(BTreeMap::from([(
        "DOTS".to_string(),
        dots_root.to_string_lossy().into_owned(),
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

    let intent_plan = derive_intent_plan(&loaded_root, &context)?;

    let source_reader = FnSourceAvailabilityReader(readable_source_path);
    let operation_context = OperationContext {
        home_directory: PathBuf::from("/home/tester"),
        source_reader: &source_reader,
        global_selection_excludes: vec![],
    };
    let operation_plan = derive_operation_plan(&intent_plan, &operation_context)?;

    Ok((intent_plan, operation_plan))
}

fn seed_dots_repository(dots_root: &Path) {
    fs::create_dir_all(dots_root).expect("dots repository should be created");
    fs::write(dots_root.join(".gitignore"), "target/\n")
        .expect("dot ignore file should be written");
    fs::write(dots_root.join(".zshrc"), "export TEST=1\n").expect("zshrc should be written");
    fs::write(dots_root.join(".vimrc"), "set number\n").expect("vimrc should be written");
    fs::write(dots_root.join("notes.txt"), "notes\n").expect("notes should be written");
}

fn shorthand_root_config() -> &'static str {
    r#"
version: 1
repository: "/repo"
defaults:
  mode: symlink
  to: "~"
  mkdir: true
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
          type: dot
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
          type: dot
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
    let dots_root = TempDir::new().expect("temporary dots repository should be created");
    let (intent_plan, operation_plan) = derive_representative_plans(
        canonical_root_config(),
        canonical_include_config(),
        dots_root.path(),
    )
    .expect("representative config should derive both plans");

    let expected_intent_plan = IntentPlan {
        repository: "/repo".to_string(),
        sources: vec![
            IntentSource {
                from: dots_root.path().to_string_lossy().into_owned(),
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
                        selection_type: SelectionType::Dot,
                        recursive: false,
                        existing_directories_only: false,
                        include: vec![],
                        exclude: validated_test_globs(&[".gitignore"]),
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

    let expected_operation_plan = available_operation_plan(vec![
        OperationAction::Copy(FileOperation {
            source: dots_root.path().join(".zshrc"),
            target: PathBuf::from("/home/tester/shell-private/.zshrc"),
            mkdir: false,
            conflict_policy: ConflictPolicy::Overwrite,
        }),
        OperationAction::Symlink(FileOperation {
            source: dots_root.path().join(".vimrc"),
            target: PathBuf::from("/home/tester/dot-targets/.vimrc"),
            mkdir: false,
            conflict_policy: ConflictPolicy::OverwriteWithBackup,
        }),
        OperationAction::Symlink(FileOperation {
            source: dots_root.path().join(".zshrc"),
            target: PathBuf::from("/home/tester/dot-targets/.zshrc"),
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
    ]);

    assert_eq!(intent_plan, expected_intent_plan);
    assert_eq!(operation_plan, expected_operation_plan);
}

#[test]
fn shorthand_and_canonical_plans_are_identical() {
    let dots_root = TempDir::new().expect("temporary dots repository should be created");
    let shorthand = derive_representative_plans(
        shorthand_root_config(),
        shorthand_include_config(),
        dots_root.path(),
    )
    .expect("shorthand representative config should derive both plans");
    let canonical = derive_representative_plans(
        canonical_root_config(),
        canonical_include_config(),
        dots_root.path(),
    )
    .expect("canonical representative config should derive both plans");

    assert_eq!(shorthand, canonical);
}
