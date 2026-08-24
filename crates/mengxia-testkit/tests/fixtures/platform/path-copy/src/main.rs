use std::path::PathBuf;

use mengxia_platform_fs::FixedSqliteChildPath;

fn copy_path(token: FixedSqliteChildPath<'_>) -> PathBuf {
    token.as_ref().to_path_buf()
}

fn main() {
    let _ = copy_path;
}
