use std::path::{Path, PathBuf};

/// Expands `~` or `~/...` using `home_directory`, otherwise returns `path`
/// unchanged as a `PathBuf`.
pub(crate) fn expand_home_prefixed_or_literal_path(path: &str, home_directory: &Path) -> PathBuf {
    if path == "~" {
        home_directory.to_path_buf()
    } else if let Some(suffix) = path.strip_prefix("~/") {
        home_directory.join(suffix)
    } else {
        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::expand_home_prefixed_or_literal_path;
    use std::path::Path;

    #[test]
    fn expand_a_bare_tilde_to_home_directory() {
        let expanded = expand_home_prefixed_or_literal_path("~", Path::new("/home/dev"));
        assert_eq!(expanded, Path::new("/home/dev"));
    }

    #[test]
    fn expand_a_tilde_prefixed_path_to_home_directory() {
        let expanded =
            expand_home_prefixed_or_literal_path("~/cfg/app.yml", Path::new("/home/dev"));
        assert_eq!(expanded, Path::new("/home/dev/cfg/app.yml"));
    }

    #[test]
    fn keep_a_non_tilde_path_unchanged() {
        let expanded = expand_home_prefixed_or_literal_path("/etc/app.yml", Path::new("/home/dev"));
        assert_eq!(expanded, Path::new("/etc/app.yml"));
    }
}
