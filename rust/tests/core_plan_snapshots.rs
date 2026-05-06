use gitenv::{
    ActionMode, ConflictPolicy, FileOperation, IntentAction, IntentFileAction, IntentPlan,
    IntentSelectAction, IntentSource, OperationAction, OperationPlan, ProgramError,
    ResolvedOptions, derive_intent_plan_with_injectables, derive_operation_plan_with_injectables,
    parse_config,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn derive_representative_plans(
    root_yaml: &str,
    include_yaml: &str,
) -> Result<(IntentPlan, OperationPlan), ProgramError> {
    let root = parse_config(root_yaml)?;
    let include = parse_config(include_yaml)?;
    let environment = BTreeMap::from([("DOTS".to_string(), "/repo/dots".to_string())]);
    let known_directories = BTreeSet::from(["~/shell".to_string(), "/feature/on".to_string()]);

    let intent_plan = derive_intent_plan_with_injectables(
        &root,
        Some(Path::new("/root.yml")),
        &environment,
        &|path| known_directories.contains(path),
        &|path: &Path| {
            if path == Path::new("/inc/common.yml") {
                Ok(include.clone())
            } else {
                Err(ProgramError::ReadConfiguration {
                    path: path.to_path_buf(),
                    message: "not found".to_string(),
                })
            }
        },
    )?;

    let operation_plan = derive_operation_plan_with_injectables(
        &intent_plan,
        &|| Ok(PathBuf::from("/home/tester")),
        &|path| {
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
        },
    )?;

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
