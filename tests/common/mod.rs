//! Shared test helpers.

use std::path::PathBuf;
use std::sync::OnceLock;
use tempfile::TempDir;

static ISOLATED_HOME: OnceLock<TempDir> = OnceLock::new();

/// A throwaway HOME shared by every command in this test binary.
///
/// Without it the tests run against `~/.config/termai/app.db` — the developer's
/// real sessions — reading, migrating and writing them on every `cargo test`.
pub fn isolated_home() -> PathBuf {
    ISOLATED_HOME
        .get_or_init(|| {
            let dir = TempDir::new().expect("temp home");
            std::fs::create_dir_all(dir.path().join(".config").join("termai"))
                .expect("temp config dir");
            dir
        })
        .path()
        .to_path_buf()
}

/// The termai binary, pointed at an isolated HOME.
#[macro_export]
macro_rules! termai_cmd {
    () => {{
        let home = $crate::common::isolated_home();
        let mut cmd = assert_cmd::cargo::cargo_bin_cmd!("termai");
        cmd.env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("NO_COLOR", "1");
        cmd
    }};
}
