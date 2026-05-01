#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub version: u32,
    pub repository: String,
    pub defaults: Defaults,
    pub sources: Vec<Source>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionMode {
    Symlink,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub from: String,
    pub configs: Vec<ConfigItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigItem {
    File(FileConfig),
    Select(SelectConfig),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileConfig {
    pub file: String,
    pub as_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectConfig {
    pub dotfiles: bool,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    UnsupportedConfiguration,
}

pub fn run() -> Result<ProgramOutput, ProgramError> {
    Ok(ProgramOutput {
        message: "Hello, World!",
    })
}

#[cfg(test)]
mod tests {
    use super::{ActionMode, Config, ConfigItem, Defaults, FileConfig, SelectConfig, Source, run};

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
                from: ".".to_string(),
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
}
