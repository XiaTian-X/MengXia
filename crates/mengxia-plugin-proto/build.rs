use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use prost::Message;
use prost_types::{FileDescriptorSet, field_descriptor_proto};
use sha2::{Digest, Sha256};

const PACKAGE: &str = "mengxia.plugin.v1";

fn main() {
    for input in [
        "../../proto/plugin/v1/control.proto",
        "../../proto/plugin/v1/control.pb",
        "../../proto/plugin/v1/control.provenance",
    ] {
        println!("cargo:rerun-if-changed={input}");
    }

    let manifest = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies CARGO_MANIFEST_DIR"),
    );
    let directory = manifest.join("../../proto/plugin/v1");
    let proto = directory.join("control.proto");
    let descriptor = directory.join("control.pb");
    let provenance = directory.join("control.provenance");
    let provenance = fs::read_to_string(provenance).expect("Plugin provenance must be UTF-8");

    assert_eq!(field(&provenance, "format="), "mengxia-proto-provenance-v1");
    assert_eq!(field(&provenance, "protoc_version="), "35.1");
    assert_eq!(field(&provenance, "prost_build_version="), "0.14.4");
    verify_digest(&proto, field(&provenance, "proto_sha256="));
    verify_digest(&descriptor, field(&provenance, "descriptor_sha256="));

    let bytes = fs::read(descriptor).expect("Plugin descriptor must be readable");
    let descriptors = FileDescriptorSet::decode(bytes.as_slice())
        .expect("Plugin descriptor must decode as FileDescriptorSet");
    generate_wire_schema(&descriptors);
    let mut config = prost_build::Config::new();
    // Protocol messages can carry the session challenge. Keep every generated
    // message and oneof out of generic Debug output so a future envelope shape
    // cannot accidentally make challenge bytes observable.
    config.skip_debug(["."]);
    config
        .compile_fds(descriptors)
        .expect("committed Plugin descriptor must generate Rust");
}

fn generate_wire_schema(descriptors: &FileDescriptorSet) {
    let file = descriptors
        .file
        .iter()
        .find(|file| file.package.as_deref() == Some(PACKAGE))
        .expect("descriptor must contain the exact Plugin package");
    assert!(
        file.dependency.is_empty(),
        "Plugin protocol imports are forbidden"
    );

    let mut output = String::from(
        "pub(crate) const fn descriptor_field(kind: MessageKind, number: u64) -> Option<FieldSpec> {\n    match (kind, number) {\n",
    );
    for message in &file.message_type {
        let name = message.name.as_deref().expect("message name is required");
        let kind = rust_kind(name);
        for field in &message.field {
            let number = field.number.expect("field number is required");
            let wire = match field.r#type() {
                field_descriptor_proto::Type::Double
                | field_descriptor_proto::Type::Fixed64
                | field_descriptor_proto::Type::Sfixed64 => 1,
                field_descriptor_proto::Type::String
                | field_descriptor_proto::Type::Bytes
                | field_descriptor_proto::Type::Message => 2,
                field_descriptor_proto::Type::Float
                | field_descriptor_proto::Type::Fixed32
                | field_descriptor_proto::Type::Sfixed32 => 5,
                _ => 0,
            };
            let child = if field.r#type() == field_descriptor_proto::Type::Message {
                let target = field
                    .type_name
                    .as_deref()
                    .expect("message field must name its target")
                    .trim_start_matches('.')
                    .strip_prefix(&format!("{PACKAGE}."))
                    .expect("Plugin messages may reference only their package");
                format!("Some(MessageKind::{})", rust_kind(target))
            } else {
                "None".to_owned()
            };
            let oneof = field.oneof_index.is_some();
            writeln!(
                output,
                "        (MessageKind::{kind}, {number}) => Some(FieldSpec {{ wire: {wire}, child: {child}, oneof: {oneof} }}),"
            )
            .expect("String writes cannot fail");
        }
    }
    output.push_str("        _ => None,\n    }\n}\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    fs::write(out.join("plugin_wire_schema.rs"), output)
        .expect("generated Plugin wire schema must be writable");
}

fn rust_kind(name: &str) -> &'static str {
    match name {
        "HostEnvelope" => "HostEnvelope",
        "PluginEnvelope" => "PluginEnvelope",
        "HostHello" => "HostHello",
        "PluginHello" => "PluginHello",
        "PingRequest" => "PingRequest",
        "PingResponse" => "PingResponse",
        "ShutdownRequest" => "ShutdownRequest",
        "ShutdownResponse" => "ShutdownResponse",
        "PluginFailure" => "PluginFailure",
        _ => panic!("descriptor introduced an unreviewed Plugin message: {name}"),
    }
}

fn field<'a>(text: &'a str, prefix: &str) -> &'a str {
    let mut matches = text.lines().filter_map(|line| line.strip_prefix(prefix));
    let value = matches.next().expect("provenance field is required");
    assert!(matches.next().is_none(), "provenance fields must be unique");
    value
}

fn verify_digest(path: &Path, expected: &str) {
    assert!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "provenance digest must be lowercase SHA-256"
    );
    let digest = Sha256::digest(fs::read(path).expect("provenance input must be readable"));
    let mut actual = String::with_capacity(64);
    for byte in digest {
        write!(&mut actual, "{byte:02x}").expect("String writes cannot fail");
    }
    assert_eq!(actual, expected, "provenance digest mismatch");
}
