use std::collections::BTreeSet;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const TARGET: &str = "aarch64-apple-darwin";
const ATTESTED_XCODE_VERSION: &str = "Xcode 26.6\nBuild version 17F113";
const ATTESTED_SDK_VERSION: &str = "26.5";
const ATTESTED_CLANG_VERSION: &str = "Apple clang version 21.0.0 (clang-2100.1.1.101)";
const ATTESTED_CLANG_SHA256: &str =
    "d2e4bf622758eee1bf7267c060497fb2c41e098d37b0fca8be73898dc7e14eda";
const ATTESTED_LIBTOOL_SHA256: &str =
    "0d41e97fd26c5dd2a268ddb1a5c07b7f8f9e6f0cd28922d92b5b19aec7c42849";
const ATTESTED_ACL_HEADER_SHA256: &str =
    "9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BuildClass {
    Developer,
    Attested,
}

fn main() {
    if let Err(message) = run() {
        panic!("mengxia-platform-fs build rejected: {message}");
    }
}

fn run() -> Result<(), String> {
    for path in [
        "build.rs",
        "include/mengxia_acl_shim.h",
        "src/macos_acl_shim.c",
        "src/macos_acl_abi_probe.c",
        "tests/macos_acl_shim_test.c",
        "../../docs/provenance/macos-acl-ffi-toolchain-v1.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=MENGXIA_ACL_BUILD_CLASS");

    let class = build_class()?;
    reject_ambient_overrides(class)?;
    require_cargo_value("HOST", TARGET)?;
    require_cargo_value("TARGET", TARGET)?;
    let out_dir = required_absolute_directory("OUT_DIR")?;
    let manifest_dir = required_absolute_directory("CARGO_MANIFEST_DIR")?;
    let repository_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "CARGO_MANIFEST_DIR has no repository root".to_owned())?;

    validate_exact_root_and_applications()?;
    for tool in [
        "/usr/bin/id",
        "/usr/bin/xcode-select",
        "/usr/bin/xcodebuild",
        "/usr/bin/xcrun",
    ] {
        validate_root_owned_system_path(Path::new(tool))?;
    }
    let identity = discover_identity()?;
    validate_build_host_identity(&identity)?;

    let logical_developer = command_text("/usr/bin/xcode-select", &["-p"])?;
    let logical_developer = PathBuf::from(logical_developer.trim());
    if ![
        Path::new("/Applications/Xcode.app/Contents/Developer"),
        Path::new("/Applications/Xcode_26.6.app/Contents/Developer"),
    ]
    .contains(&logical_developer.as_path())
    {
        return Err("selected developer directory is outside the closed allowlist".to_owned());
    }
    let canonical_developer = fs::canonicalize(&logical_developer)
        .map_err(|_| "selected developer directory cannot be canonicalized".to_owned())?;
    let canonical_allowed = [
        Path::new("/Applications/Xcode.app/Contents/Developer"),
        Path::new("/Applications/Xcode_26.6.app/Contents/Developer"),
    ]
    .iter()
    .filter_map(|path| fs::canonicalize(path).ok())
    .any(|path| path == canonical_developer);
    if !canonical_allowed {
        return Err("canonical developer directory is outside the closed allowlist".to_owned());
    }
    validate_selected_xcode_path(&logical_developer, &canonical_developer, identity.euid)?;

    let sdk_path = PathBuf::from(
        command_text(
            "/usr/bin/xcrun",
            &["--no-cache", "--sdk", "macosx", "--show-sdk-path"],
        )?
        .trim(),
    );
    let clang = PathBuf::from(
        command_text(
            "/usr/bin/xcrun",
            &["--no-cache", "--sdk", "macosx", "--find", "clang"],
        )?
        .trim(),
    );
    let libtool = PathBuf::from(
        command_text(
            "/usr/bin/xcrun",
            &["--no-cache", "--sdk", "macosx", "--find", "libtool"],
        )?
        .trim(),
    );
    for path in [&sdk_path, &clang, &libtool] {
        if path.starts_with(&canonical_developer) {
            validate_owned_nonwritable_chain(&canonical_developer, path, identity.euid)?;
        }
        let canonical = fs::canonicalize(path)
            .map_err(|_| "selected Xcode component cannot be canonicalized".to_owned())?;
        if !canonical.starts_with(&canonical_developer) {
            return Err("selected Xcode component escaped the developer directory".to_owned());
        }
        validate_owned_nonwritable_chain(&canonical_developer, &canonical, identity.euid)?;
    }
    let acl_header = sdk_path.join("usr/include/sys/acl.h");
    if acl_header.starts_with(&canonical_developer) {
        validate_owned_nonwritable_chain(&canonical_developer, &acl_header, identity.euid)?;
    }
    let canonical_acl_header = fs::canonicalize(&acl_header)
        .map_err(|_| "SDK sys/acl.h cannot be canonicalized".to_owned())?;
    if !canonical_acl_header.starts_with(&canonical_developer) {
        return Err("SDK sys/acl.h escaped the developer directory".to_owned());
    }
    validate_owned_nonwritable_chain(&canonical_developer, &canonical_acl_header, identity.euid)?;

    let xcode_version = command_text("/usr/bin/xcodebuild", &["-version"])?;
    let sdk_version = command_text(
        "/usr/bin/xcrun",
        &["--no-cache", "--sdk", "macosx", "--show-sdk-version"],
    )?;
    let clang_version = command_path_text(&clang, &["--version"])?;
    let clang_banner = clang_version.lines().next().unwrap_or_default();
    if !clang_banner.starts_with("Apple clang version ") {
        return Err("developer build requires Apple clang".to_owned());
    }

    let clang_digest = sha256_file(&clang)?;
    let libtool_digest = sha256_file(&libtool)?;
    let acl_header_digest = sha256_file(&acl_header)?;
    if class == BuildClass::Attested
        && (xcode_version.trim() != ATTESTED_XCODE_VERSION
            || sdk_version.trim() != ATTESTED_SDK_VERSION
            || clang_banner != ATTESTED_CLANG_VERSION
            || clang_digest != ATTESTED_CLANG_SHA256
            || libtool_digest != ATTESTED_LIBTOOL_SHA256
            || acl_header_digest != ATTESTED_ACL_HEADER_SHA256)
    {
        return Err("attested toolchain tuple or digest drifted".to_owned());
    }

    let include_dir = manifest_dir.join("include");
    let shim_source = manifest_dir.join("src/macos_acl_shim.c");
    let probe_source = manifest_dir.join("src/macos_acl_abi_probe.c");
    let test_source = manifest_dir.join("tests/macos_acl_shim_test.c");
    let shim_object = out_dir.join("macos_acl_shim.o");
    let probe_object = out_dir.join("macos_acl_abi_probe.o");
    let test_object = out_dir.join("macos_acl_shim_test.o");
    let test_executable = out_dir.join("macos_acl_shim_test");
    let archive = out_dir.join("libmengxia_acl_shim.a");
    let source_digests = [
        (
            "include/mengxia_acl_shim.h",
            sha256_file(&include_dir.join("mengxia_acl_shim.h"))?,
        ),
        ("src/macos_acl_shim.c", sha256_file(&shim_source)?),
        ("src/macos_acl_abi_probe.c", sha256_file(&probe_source)?),
        ("tests/macos_acl_shim_test.c", sha256_file(&test_source)?),
    ];
    verify_manifest_source_digests(repository_root, &source_digests)?;

    let common_args = vec![
        "-target".to_owned(),
        "arm64-apple-macos13.0".to_owned(),
        "-isysroot".to_owned(),
        sdk_path.to_string_lossy().into_owned(),
        "-I".to_owned(),
        include_dir.to_string_lossy().into_owned(),
        "-std=c11".to_owned(),
        "-fvisibility=hidden".to_owned(),
        "-fno-common".to_owned(),
        "-fPIC".to_owned(),
        "-fstack-protector-strong".to_owned(),
        "-O2".to_owned(),
        "-g0".to_owned(),
        "-D_FORTIFY_SOURCE=2".to_owned(),
        "-Wall".to_owned(),
        "-Wextra".to_owned(),
        "-Wpedantic".to_owned(),
        "-Wconversion".to_owned(),
        "-Wsign-conversion".to_owned(),
        "-Wshadow".to_owned(),
        "-Wstrict-prototypes".to_owned(),
        "-Wmissing-prototypes".to_owned(),
        "-Werror".to_owned(),
    ];
    let shim_argv = compile_source(&clang, &common_args, &shim_source, &shim_object)?;
    let probe_argv = compile_source(&clang, &common_args, &probe_source, &probe_object)?;
    let test_argv = compile_source(&clang, &common_args, &test_source, &test_object)?;
    let test_link_argv = vec![
        "-target".to_owned(),
        "arm64-apple-macos13.0".to_owned(),
        "-isysroot".to_owned(),
        sdk_path.to_string_lossy().into_owned(),
        shim_object.to_string_lossy().into_owned(),
        test_object.to_string_lossy().into_owned(),
        "-o".to_owned(),
        test_executable.to_string_lossy().into_owned(),
    ];
    run_tool(&clang, &test_link_argv, false)?;
    run_tool(&test_executable, &[], false)?;
    let archive_argv = vec![
        "-static".to_owned(),
        "-o".to_owned(),
        archive.to_string_lossy().into_owned(),
        shim_object.to_string_lossy().into_owned(),
    ];
    run_tool(&libtool, &archive_argv, true)?;
    let output_digests = [
        ("macos_acl_shim.o", sha256_file(&shim_object)?),
        ("macos_acl_abi_probe.o", sha256_file(&probe_object)?),
        ("macos_acl_shim_test.o", sha256_file(&test_object)?),
        ("macos_acl_shim_test", sha256_file(&test_executable)?),
        ("libmengxia_acl_shim.a", sha256_file(&archive)?),
    ];

    validate_selected_xcode_path(&logical_developer, &canonical_developer, identity.euid)?;
    for path in [&clang, &libtool, &acl_header] {
        if path.starts_with(&canonical_developer) {
            validate_owned_nonwritable_chain(&canonical_developer, path, identity.euid)?;
        }
        let canonical = fs::canonicalize(path)
            .map_err(|_| "Xcode component changed after archive creation".to_owned())?;
        validate_owned_nonwritable_chain(&canonical_developer, &canonical, identity.euid)?;
    }
    if sha256_file(&clang)? != clang_digest
        || sha256_file(&libtool)? != libtool_digest
        || sha256_file(&acl_header)? != acl_header_digest
    {
        return Err("toolchain input changed during archive creation".to_owned());
    }

    let evidence_path = out_dir.join("mengxia-acl-build-command-v1.json");
    let evidence = build_evidence_json(
        class,
        &logical_developer,
        &canonical_developer,
        &sdk_path,
        &clang,
        &libtool,
        &clang_digest,
        &libtool_digest,
        &acl_header_digest,
        &xcode_version,
        &sdk_version,
        &clang_version,
        &source_digests,
        &output_digests,
        &shim_argv,
        &probe_argv,
        &test_argv,
        &test_link_argv,
        &archive_argv,
        identity,
    );
    fs::write(&evidence_path, evidence).map_err(|_| "failed to write build evidence".to_owned())?;

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=mengxia_acl_shim");
    Ok(())
}

#[derive(Clone, Copy)]
struct Identity {
    euid: u32,
    primary_gid: u32,
    groups_include_admin: bool,
}

fn build_class() -> Result<BuildClass, String> {
    match env::var("MENGXIA_ACL_BUILD_CLASS") {
        Err(env::VarError::NotPresent) => Ok(BuildClass::Developer),
        Ok(value) if value == "attested" => Ok(BuildClass::Attested),
        Ok(_) | Err(env::VarError::NotUnicode(_)) => {
            Err("MENGXIA_ACL_BUILD_CLASS is invalid".to_owned())
        }
    }
}

fn reject_ambient_overrides(class: BuildClass) -> Result<(), String> {
    let always = [
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
        "NM",
        "STRIP",
        "ZERO_AR_DATE",
        "MENGXIA_ACL_TESTING",
    ];
    let attested = [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS",
        "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER",
        "BINDGEN_EXTRA_CLANG_ARGS",
        "CLANG_PATH",
        "COMPILER_PATH",
    ];
    let target_forms = [
        "AARCH64_APPLE_DARWIN_CC",
        "AARCH64_APPLE_DARWIN_CFLAGS",
        "AARCH64_APPLE_DARWIN_CPPFLAGS",
        "AARCH64_APPLE_DARWIN_AR",
        "AARCH64_APPLE_DARWIN_ARFLAGS",
        "aarch64_apple_darwin_CC",
        "aarch64_apple_darwin_CFLAGS",
        "aarch64_apple_darwin_CPPFLAGS",
        "aarch64_apple_darwin_AR",
        "aarch64_apple_darwin_ARFLAGS",
        "aarch64-apple-darwin_CC",
        "aarch64-apple-darwin_CFLAGS",
        "aarch64-apple-darwin_CPPFLAGS",
        "aarch64-apple-darwin_AR",
        "aarch64-apple-darwin_ARFLAGS",
        "HOST_CC",
        "HOST_CFLAGS",
        "HOST_CPPFLAGS",
        "HOST_AR",
        "HOST_ARFLAGS",
        "TARGET_CC",
        "TARGET_CFLAGS",
        "TARGET_CPPFLAGS",
        "TARGET_AR",
        "TARGET_ARFLAGS",
    ];
    for (key, value) in env::vars_os() {
        let key_text = key.to_string_lossy();
        let rejected = always.contains(&key_text.as_ref())
            || target_forms.contains(&key_text.as_ref())
            || key_text.starts_with("CRATE_CC_")
            || key_text.starts_with("CC_")
            || key_text.starts_with("CFLAGS_")
            || key_text.starts_with("CPPFLAGS_")
            || (key_text.starts_with("MENGXIA_ACL_") && key_text != "MENGXIA_ACL_BUILD_CLASS")
            || (class == BuildClass::Attested
                && attested.contains(&key_text.as_ref())
                && !value.is_empty());
        if rejected {
            return Err(format!("ambient override {key_text} is forbidden"));
        }
    }
    Ok(())
}

fn require_cargo_value(key: &str, expected: &str) -> Result<(), String> {
    match env::var(key) {
        Ok(value) if value == expected => Ok(()),
        _ => Err(format!("{key} must be exactly {expected}")),
    }
}

fn required_absolute_directory(key: &str) -> Result<PathBuf, String> {
    let value = env::var_os(key).ok_or_else(|| format!("{key} is missing"))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() || !path.is_dir() {
        return Err(format!("{key} is not an absolute existing directory"));
    }
    Ok(path)
}

fn discover_identity() -> Result<Identity, String> {
    let euid = strict_decimal(&command_text("/usr/bin/id", &["-u"])?)?;
    let primary_gid = strict_decimal(&command_text("/usr/bin/id", &["-g"])?)?;
    let groups_text = command_text("/usr/bin/id", &["-G"])?;
    let mut groups = BTreeSet::new();
    for field in groups_text.split_ascii_whitespace() {
        let group = strict_decimal(field)?;
        if !groups.insert(group) {
            return Err("id -G returned a duplicate group".to_owned());
        }
    }
    if groups.is_empty() || !groups.contains(&primary_gid) {
        return Err("id -G did not contain the primary GID".to_owned());
    }
    Ok(Identity {
        euid,
        primary_gid,
        groups_include_admin: groups.contains(&80),
    })
}

fn strict_decimal(value: &str) -> Result<u32, String> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("identity command returned malformed decimal output".to_owned());
    }
    value
        .parse()
        .map_err(|_| "identity command returned an out-of-range value".to_owned())
}

fn validate_build_host_identity(identity: &Identity) -> Result<(), String> {
    if identity.euid != 0 && !identity.groups_include_admin {
        return Err("non-root build account is not a member of numeric GID 80".to_owned());
    }
    Ok(())
}

fn validate_exact_root_and_applications() -> Result<(), String> {
    validate_exact_directory(Path::new("/"), 0, 0, 0o755)?;
    validate_exact_directory(Path::new("/Applications"), 0, 80, 0o775)
}

fn validate_exact_directory(path: &Path, uid: u32, gid: u32, mode: u32) -> Result<(), String> {
    let link = fs::symlink_metadata(path).map_err(|_| "build path metadata failed".to_owned())?;
    if link.file_type().is_symlink()
        || !link.is_dir()
        || link.uid() != uid
        || link.gid() != gid
        || link.permissions().mode() & 0o7777 != mode
    {
        return Err("build-host permission matrix rejected a fixed component".to_owned());
    }
    Ok(())
}

fn validate_root_owned_system_path(path: &Path) -> Result<(), String> {
    let mut current = PathBuf::from("/");
    for component in path.components().skip(1) {
        current.push(component);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|_| "system-tool path metadata failed".to_owned())?;
        if metadata.file_type().is_symlink()
            || metadata.uid() != 0
            || metadata.permissions().mode() & 0o022 != 0
        {
            return Err("system-tool path is not root-owned and non-writable".to_owned());
        }
    }
    Ok(())
}

fn validate_selected_xcode_path(
    logical_developer: &Path,
    canonical_developer: &Path,
    euid: u32,
) -> Result<(), String> {
    let logical_bundle = logical_developer
        .ancestors()
        .find(|path| path.parent() == Some(Path::new("/Applications")))
        .ok_or_else(|| "logical Xcode bundle is malformed".to_owned())?;
    let link = fs::symlink_metadata(logical_bundle)
        .map_err(|_| "logical Xcode bundle metadata failed".to_owned())?;
    if link.uid() != 0 && link.uid() != euid {
        return Err("logical Xcode bundle owner is outside the accepted set".to_owned());
    }
    if link.permissions().mode() & 0o022 != 0 {
        return Err("logical Xcode bundle is group/world writable".to_owned());
    }
    validate_owned_nonwritable_chain(
        canonical_developer
            .ancestors()
            .find(|path| path.parent() == Some(Path::new("/Applications")))
            .ok_or_else(|| "canonical Xcode bundle is malformed".to_owned())?,
        canonical_developer,
        euid,
    )
}

fn validate_owned_nonwritable_chain(root: &Path, target: &Path, euid: u32) -> Result<(), String> {
    if !target.starts_with(root) {
        return Err("build component escaped its accepted root".to_owned());
    }
    let relative = target
        .strip_prefix(root)
        .map_err(|_| "build component prefix mismatch".to_owned())?;
    let mut current = root.to_path_buf();
    validate_owned_nonwritable_component(&current, euid)?;
    for component in relative.components() {
        current.push(component);
        let link = fs::symlink_metadata(&current)
            .map_err(|_| "build component metadata failed".to_owned())?;
        if link.file_type().is_symlink() {
            let canonical = fs::canonicalize(&current)
                .map_err(|_| "build component symlink is broken".to_owned())?;
            if !canonical.starts_with(root) {
                return Err("build component symlink escaped the accepted bundle".to_owned());
            }
        }
        validate_owned_nonwritable_metadata(&link, euid)?;
    }
    Ok(())
}

fn validate_owned_nonwritable_component(path: &Path, euid: u32) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "build component metadata failed".to_owned())?;
    validate_owned_nonwritable_metadata(&metadata, euid)
}

fn validate_owned_nonwritable_metadata(metadata: &fs::Metadata, euid: u32) -> Result<(), String> {
    if metadata.uid() != 0 && metadata.uid() != euid {
        return Err("build component owner is outside the accepted set".to_owned());
    }
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err("build component is group/world writable".to_owned());
    }
    Ok(())
}

fn command_text(program: &str, args: &[&str]) -> Result<String, String> {
    command_path_text(Path::new(program), args)
}

fn command_path_text(program: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args).env_clear().env("LC_ALL", "C");
    checked_output(command, program)
}

fn checked_output(mut command: Command, program: &Path) -> Result<String, String> {
    let output = command
        .output()
        .map_err(|_| format!("failed to execute {}", program.display()))?;
    // Developer builds may run inside a filesystem sandbox where Xcode emits
    // cache/FSEvents diagnostics on stderr. Authority comes from the strictly
    // parsed stdout plus the subsequent path, identity, ABI and digest checks.
    if !output.status.success() {
        return Err(format!("{} returned a rejected result", program.display()));
    }
    if output.stdout.len() > 16 * 1024 {
        return Err(format!("{} returned oversized output", program.display()));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| format!("{} returned non-UTF-8 output", program.display()))
}

fn compile_source(
    clang: &Path,
    common_args: &[String],
    source: &Path,
    object: &Path,
) -> Result<Vec<String>, String> {
    let mut args = common_args.to_vec();
    args.extend([
        "-c".to_owned(),
        source.to_string_lossy().into_owned(),
        "-o".to_owned(),
        object.to_string_lossy().into_owned(),
    ]);
    run_tool(clang, &args, false)?;
    Ok(args)
}

fn run_tool(program: &Path, args: &[String], archive: bool) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(args).env_clear().env("LC_ALL", "C");
    if archive {
        command.env("ZERO_AR_DATE", "1");
    }
    let Output {
        status,
        stdout,
        stderr,
    } = command
        .output()
        .map_err(|_| format!("failed to execute {}", program.display()))?;
    if !status.success() || !stdout.is_empty() || !stderr.is_empty() {
        return Err(format!(
            "{} rejected the checked-in argv",
            program.display()
        ));
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|_| format!("failed to read {}", path.display()))?;
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}")
            .map_err(|_| "failed to encode a SHA-256 digest".to_owned())?;
    }
    Ok(encoded)
}

fn verify_manifest_source_digests(
    repository_root: &Path,
    digests: &[(&str, String); 4],
) -> Result<(), String> {
    let path = repository_root.join("docs/provenance/macos-acl-ffi-toolchain-v1.toml");
    let manifest = fs::read_to_string(path)
        .map_err(|_| "ACL toolchain provenance manifest is missing".to_owned())?;
    for (name, digest) in digests {
        let expected = format!("\"{name}\" = \"{digest}\"");
        if !manifest.lines().any(|line| line == expected) {
            return Err(format!("provenance digest is missing or stale for {name}"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_evidence_json(
    class: BuildClass,
    logical_developer: &Path,
    canonical_developer: &Path,
    sdk: &Path,
    clang: &Path,
    libtool: &Path,
    clang_digest: &str,
    libtool_digest: &str,
    acl_header_digest: &str,
    xcode_version: &str,
    sdk_version: &str,
    clang_version: &str,
    source_digests: &[(&str, String); 4],
    output_digests: &[(&str, String); 5],
    shim_argv: &[String],
    probe_argv: &[String],
    test_argv: &[String],
    test_link_argv: &[String],
    archive_argv: &[String],
    identity: Identity,
) -> String {
    format!(
        concat!(
            "{{\n  \"schema\": 1,\n  \"class\": \"{}\",\n",
            "  \"attested\": {},\n  \"euid\": {},\n  \"primary_gid\": {},\n",
            "  \"logical_developer\": \"{}\",\n  \"canonical_developer\": \"{}\",\n",
            "  \"sdk\": \"{}\",\n  \"clang\": \"{}\",\n  \"libtool\": \"{}\",\n",
            "  \"clang_sha256\": \"{}\",\n  \"libtool_sha256\": \"{}\",\n",
            "  \"acl_header_sha256\": \"{}\",\n",
            "  \"xcode_version\": \"{}\",\n  \"sdk_version\": \"{}\",\n",
            "  \"clang_version\": \"{}\",\n  \"source_digests\": {},\n",
            "  \"output_digests\": {},\n  \"compile_shim_argv\": {},\n",
            "  \"compile_probe_argv\": {},\n",
            "  \"compile_test_argv\": {},\n  \"link_test_argv\": {},\n",
            "  \"archive_argv\": {},\n",
            "  \"child_environment\": {{\"LC_ALL\":\"C\",\"ZERO_AR_DATE_archive_only\":\"1\"}}\n}}\n"
        ),
        match class {
            BuildClass::Developer => "developer",
            BuildClass::Attested => "attested",
        },
        class == BuildClass::Attested,
        identity.euid,
        identity.primary_gid,
        json_escape(logical_developer.as_os_str()),
        json_escape(canonical_developer.as_os_str()),
        json_escape(sdk.as_os_str()),
        json_escape(clang.as_os_str()),
        json_escape(libtool.as_os_str()),
        clang_digest,
        libtool_digest,
        acl_header_digest,
        json_escape(xcode_version.trim().as_ref()),
        json_escape(sdk_version.trim().as_ref()),
        json_escape(clang_version.trim().as_ref()),
        json_digest_map(source_digests),
        json_digest_map(output_digests),
        json_array(shim_argv),
        json_array(probe_argv),
        json_array(test_argv),
        json_array(test_link_argv),
        json_array(archive_argv),
    )
}

fn json_array(values: &[String]) -> String {
    let values: Vec<_> = values
        .iter()
        .map(|value| format!("\"{}\"", json_escape(value.as_ref())))
        .collect();
    format!("[{}]", values.join(","))
}

fn json_digest_map<const N: usize>(values: &[(&str, String); N]) -> String {
    let values: Vec<_> = values
        .iter()
        .map(|(name, digest)| format!("\"{}\":\"{}\"", json_escape(name.as_ref()), digest))
        .collect();
    format!("{{{}}}", values.join(","))
}

fn json_escape(value: &OsStr) -> String {
    value
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
