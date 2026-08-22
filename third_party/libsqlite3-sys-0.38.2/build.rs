use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REJECTED_ENVIRONMENT: &[&str] = &[
    "LIBSQLITE3_SYS_USE_PKG_CONFIG",
    "LIBSQLITE3_FLAGS",
    "SQLITE3_LIB_DIR",
    "SQLITE3_INCLUDE_DIR",
    "SQLITE3_STATIC",
    "SQLITE_MAX_VARIABLE_NUMBER",
    "SQLITE_MAX_EXPR_DEPTH",
    "SQLITE_MAX_COLUMN",
    "CC",
    "CFLAGS",
    "CPPFLAGS",
    "CPATH",
    "C_INCLUDE_PATH",
    "CPLUS_INCLUDE_PATH",
    "OBJC_INCLUDE_PATH",
    "SDKROOT",
    "DEVELOPER_DIR",
    "TOOLCHAINS",
    "MACOSX_DEPLOYMENT_TARGET",
    "ARCHFLAGS",
    "LD",
    "LDFLAGS",
    "LIBRARY_PATH",
    "AR",
    "ARFLAGS",
    "RANLIB",
    "RANLIBFLAGS",
    "ZERO_AR_DATE",
];

const SQLITE_DEFINES: &[&str] = &[
    "-DSQLITE_THREADSAFE=1",
    "-DSQLITE_DQS=0",
    "-DSQLITE_DEFAULT_FOREIGN_KEYS=1",
    "-DSQLITE_DEFAULT_WAL_SYNCHRONOUS=2",
    "-DSQLITE_TRUSTED_SCHEMA=0",
    "-DSQLITE_OMIT_LOAD_EXTENSION",
];

fn main() {
    emit_rerun_rules();
    reject_unsupported_features();
    reject_environment();

    let target = required_env("TARGET");
    let host = required_env("HOST");
    assert_eq!(target, "aarch64-apple-darwin", "unsupported SQLite target");
    assert_eq!(host, "aarch64-apple-darwin", "unsupported SQLite host");

    let out_dir = PathBuf::from(required_env("OUT_DIR"));
    fs::copy(
        "sqlite3/bindgen_bundled_version.rs",
        out_dir.join("bindgen.rs"),
    )
    .expect("copy checked-in SQLite bindings");

    let sdk_root = utf8_line(run_xcrun(["--show-sdk-path"]), "SDK root");
    let clang = utf8_line(run_xcrun(["--find", "clang"]), "clang path");
    let libtool = utf8_line(run_xcrun(["--find", "libtool"]), "libtool path");
    require_absolute_existing_file(&clang, "clang");
    require_absolute_existing_file(&libtool, "libtool");
    require_absolute_existing_dir(&sdk_root, "SDK root");

    let object = out_dir.join("sqlite3.o");
    let archive = out_dir.join("libsqlite3.a");
    let source = Path::new("sqlite3/sqlite3.c");

    let mut compile = clean_command(&clang);
    compile
        .args([
            "-target",
            "arm64-apple-macos13.0",
            "-isysroot",
            &sdk_root,
            "-std=c11",
            "-fvisibility=hidden",
            "-fno-common",
            "-fPIC",
            "-fstack-protector-strong",
            "-O2",
            "-g0",
            "-D_FORTIFY_SOURCE=2",
        ])
        .args(SQLITE_DEFINES)
        .arg("-c")
        .arg(source)
        .arg("-o")
        .arg(&object);
    run_checked(compile, "compile source-pinned SQLite");

    let mut archive_command = clean_command(&libtool);
    archive_command
        .env("ZERO_AR_DATE", "1")
        .args(["-static", "-o"])
        .arg(&archive)
        .arg(&object);
    run_checked(archive_command, "archive source-pinned SQLite");

    println!("cargo:include={}", Path::new("sqlite3").display());
    println!("cargo:lib_dir={}", out_dir.display());
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sqlite3");
}

fn emit_rerun_rules() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=sqlite3/sqlite3.c");
    println!("cargo:rerun-if-changed=sqlite3/sqlite3.h");
    println!("cargo:rerun-if-changed=sqlite3/bindgen_bundled_version.rs");
    for variable in REJECTED_ENVIRONMENT {
        println!("cargo:rerun-if-env-changed={variable}");
    }
}

fn reject_unsupported_features() {
    for (name, enabled) in [
        ("buildtime_bindgen", cfg!(feature = "buildtime_bindgen")),
        ("bundled-sqlcipher", cfg!(feature = "bundled-sqlcipher")),
        (
            "bundled-sqlcipher-vendored-openssl",
            cfg!(feature = "bundled-sqlcipher-vendored-openssl"),
        ),
        ("bundled-windows", cfg!(feature = "bundled-windows")),
        ("column_metadata", cfg!(feature = "column_metadata")),
        ("in_gecko", cfg!(feature = "in_gecko")),
        ("loadable_extension", cfg!(feature = "loadable_extension")),
        ("preupdate_hook", cfg!(feature = "preupdate_hook")),
        ("session", cfg!(feature = "session")),
        ("sqlcipher", cfg!(feature = "sqlcipher")),
        ("unlock_notify", cfg!(feature = "unlock_notify")),
        ("wasm32-wasi-vfs", cfg!(feature = "wasm32-wasi-vfs")),
        ("with-asan", cfg!(feature = "with-asan")),
    ] {
        assert!(!enabled, "unsupported libsqlite3-sys feature: {name}");
    }
    assert!(cfg!(feature = "bundled"), "bundled SQLite is mandatory");
}

fn reject_environment() {
    for variable in REJECTED_ENVIRONMENT {
        assert!(
            env::var_os(variable).is_none(),
            "environment override is forbidden: {variable}"
        );
    }
}

fn required_env(name: &str) -> OsString {
    env::var_os(name).unwrap_or_else(|| panic!("missing Cargo environment: {name}"))
}

fn clean_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    command.env_clear().env("LC_ALL", "C").env("LANG", "C");
    command
}

fn run_xcrun<const N: usize>(arguments: [&str; N]) -> Output {
    let mut command = clean_command("/usr/bin/xcrun");
    command.args(["--no-cache", "--sdk", "macosx"]);
    command.args(arguments);
    run_output(command, "resolve Apple tool")
}

fn run_checked(mut command: Command, description: &str) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(
        output.status.success(),
        "{description} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_output(mut command: Command, description: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{description}: {error}"));
    assert!(
        output.status.success(),
        "{description} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn utf8_line(output: Output, description: &str) -> String {
    let value = String::from_utf8(output.stdout)
        .unwrap_or_else(|error| panic!("{description} is not UTF-8: {error}"));
    let trimmed = value.trim_end_matches(['\r', '\n']);
    assert!(
        !trimmed.is_empty() && !trimmed.contains(['\r', '\n']),
        "{description} must be exactly one nonempty line"
    );
    trimmed.to_owned()
}

fn require_absolute_existing_file(path: &str, description: &str) {
    let path = Path::new(path);
    assert!(path.is_absolute(), "{description} path is not absolute");
    assert!(path.is_file(), "{description} path is not a file");
}

fn require_absolute_existing_dir(path: &str, description: &str) {
    let path = Path::new(path);
    assert!(path.is_absolute(), "{description} path is not absolute");
    assert!(path.is_dir(), "{description} path is not a directory");
}
