use clap::ValueEnum;

/// Controls ANSI color output in CLI rendering and diagnostic logs.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    /// Enable color only when output is a terminal.
    Auto,
    /// Always enable color.
    Yes,
    /// Always disable color.
    No,
}

/// Precomputed color decision state for stdout and stderr.
/// Built once at program startup to avoid repeated terminal detection.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeConfig {
    /// Whether to use color when writing to stdout.
    pub use_color_for_stdout: bool,
    /// Whether to use color when writing to stderr (used by logs).
    pub use_color_for_stderr: bool,
}

impl RuntimeConfig {
    /// Build RuntimeConfig from a ColorMode and terminal detection functions.
    ///
    /// Precomputes color decisions once at startup; the resulting booleans
    /// are then used throughout the program without further terminal probes.
    pub(crate) fn new(
        color_mode: ColorMode,
        stdout_is_terminal: bool,
        stderr_is_terminal: bool,
    ) -> Self {
        RuntimeConfig {
            use_color_for_stdout: should_use_color(color_mode, stdout_is_terminal),
            use_color_for_stderr: should_use_color(color_mode, stderr_is_terminal),
        }
    }
}

fn should_use_color(color_mode: ColorMode, is_terminal: bool) -> bool {
    match color_mode {
        ColorMode::Auto => is_terminal,
        ColorMode::Yes => true,
        ColorMode::No => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorMode, RuntimeConfig, should_use_color};

    #[test]
    fn use_terminal_detection_in_auto_mode() {
        let use_color_when_terminal = should_use_color(ColorMode::Auto, true);
        let use_color_when_not_terminal = should_use_color(ColorMode::Auto, false);

        assert!(use_color_when_terminal);
        assert!(!use_color_when_not_terminal);
    }

    #[test]
    fn always_enable_color_in_yes_mode() {
        assert!(should_use_color(ColorMode::Yes, false));
        assert!(should_use_color(ColorMode::Yes, true));
    }

    #[test]
    fn always_disable_color_in_no_mode() {
        assert!(!should_use_color(ColorMode::No, false));
        assert!(!should_use_color(ColorMode::No, true));
    }

    #[test]
    fn runtime_config_precomputes_color_decisions() {
        let config_auto_terminal = RuntimeConfig::new(ColorMode::Auto, true, true);
        assert!(config_auto_terminal.use_color_for_stdout);
        assert!(config_auto_terminal.use_color_for_stderr);

        let config_auto_no_terminal = RuntimeConfig::new(ColorMode::Auto, false, false);
        assert!(!config_auto_no_terminal.use_color_for_stdout);
        assert!(!config_auto_no_terminal.use_color_for_stderr);

        let config_yes = RuntimeConfig::new(ColorMode::Yes, false, false);
        assert!(config_yes.use_color_for_stdout);
        assert!(config_yes.use_color_for_stderr);

        let config_no = RuntimeConfig::new(ColorMode::No, true, true);
        assert!(!config_no.use_color_for_stdout);
        assert!(!config_no.use_color_for_stderr);
    }

    #[test]
    fn runtime_config_handles_different_terminal_states() {
        let config = RuntimeConfig::new(ColorMode::Auto, true, false);
        assert!(config.use_color_for_stdout);
        assert!(!config.use_color_for_stderr);
    }
}
