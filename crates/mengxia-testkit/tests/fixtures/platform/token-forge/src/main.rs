use std::path::Path;

use mengxia_platform_fs::FixedSqliteChildPath;

fn main() {
    let _forged = FixedSqliteChildPath {
        path: Path::new("/tmp/attacker-selected.sqlite3"),
    };
}
