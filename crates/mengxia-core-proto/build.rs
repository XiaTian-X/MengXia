use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use prost::Message;
use prost_types::{DescriptorProto, FileDescriptorSet, field_descriptor_proto};
use sha2::{Digest, Sha256};

const PROTO_SHA_KEY: &str = "proto_sha256=";
const DESCRIPTOR_SHA_KEY: &str = "descriptor_sha256=";

fn main() {
    println!("cargo:rerun-if-changed=../../proto/core/v1/handshake.proto");
    println!("cargo:rerun-if-changed=../../proto/core/v1/handshake.pb");
    println!("cargo:rerun-if-changed=../../proto/core/v1/handshake.provenance");

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo supplies CARGO_MANIFEST_DIR"),
    );
    let proto_dir = manifest_dir.join("../../proto/core/v1");
    let proto = proto_dir.join("handshake.proto");
    let descriptor = proto_dir.join("handshake.pb");
    let provenance = proto_dir.join("handshake.provenance");

    let provenance_text =
        fs::read_to_string(provenance).expect("committed proto provenance must be readable UTF-8");
    assert_eq!(
        field(&provenance_text, "format="),
        "mengxia-proto-provenance-v1",
        "proto provenance format must be exact"
    );
    assert_eq!(field(&provenance_text, "protoc_version="), "35.1");
    assert_eq!(field(&provenance_text, "prost_build_version="), "0.14.4");

    verify_digest(&proto, field(&provenance_text, PROTO_SHA_KEY));
    verify_digest(&descriptor, field(&provenance_text, DESCRIPTOR_SHA_KEY));

    let descriptor_bytes = fs::read(descriptor).expect("committed descriptor set must be readable");
    let descriptor_set = FileDescriptorSet::decode(descriptor_bytes.as_slice())
        .expect("committed descriptor set must decode");
    generate_depth_table(&descriptor_set);
    prost_build::Config::new()
        .compile_fds(descriptor_set)
        .expect("committed descriptor set must generate Rust");
}

fn generate_depth_table(descriptor_set: &FileDescriptorSet) {
    let file = descriptor_set
        .file
        .iter()
        .find(|file| file.package.as_deref() == Some("mengxia.core.v1"))
        .expect("descriptor must contain the canonical package");
    let package = file.package.as_deref().expect("package is present");
    let mut messages = Vec::new();
    collect_messages(package, "", &file.message_type, &mut messages);

    let mut edges = Vec::new();
    for (full_name, descriptor) in &messages {
        for field in &descriptor.field {
            if field.r#type() == field_descriptor_proto::Type::Message {
                edges.push((
                    full_name.clone(),
                    i64::from(field.number()),
                    field
                        .type_name
                        .as_deref()
                        .expect("message field must have a type name")
                        .trim_start_matches('.')
                        .to_owned(),
                ));
            }
        }
    }

    let handshake_roots = [
        "mengxia.core.v1.ClientHello",
        "mengxia.core.v1.HandshakeResponse",
    ];
    let operation_roots = [
        "mengxia.core.v1.CoreRequest",
        "mengxia.core.v1.CoreResponse",
    ];
    let handshake_maximum = handshake_roots
        .iter()
        .map(|root| message_depth(root, &edges, &mut Vec::new()))
        .max()
        .expect("at least one root");
    let operation_maximum = operation_roots
        .iter()
        .map(|root| message_depth(root, &edges, &mut Vec::new()))
        .max()
        .expect("at least one operation root");
    let maximum = handshake_maximum.max(operation_maximum);
    assert!(maximum <= 64, "descriptor message depth exceeds 64");

    let mut generated = format!(
        "const HANDSHAKE_DESCRIPTOR_MAX_DEPTH: u8 = {handshake_maximum};\n\
         const OPERATION_DESCRIPTOR_MAX_DEPTH: u8 = {operation_maximum};\n\
         pub const DESCRIPTOR_MAX_DEPTH: u8 = {maximum};\n"
    );
    generated.push_str(
        "const fn descriptor_embedded_message(kind: MessageKind, field_number: u64) -> Option<MessageKind> {\n    match (kind, field_number) {\n",
    );
    for (source, number, target) in edges {
        let source = rust_message_kind(&source);
        let target = rust_message_kind(&target);
        writeln!(
            &mut generated,
            "        (MessageKind::{source}, {number}) => Some(MessageKind::{target}),"
        )
        .expect("writing to String cannot fail");
    }
    generated.push_str("        _ => None,\n    }\n}\n");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    fs::write(out_dir.join("mengxia.depth.rs"), generated)
        .expect("generated depth table must be writable");
}

fn collect_messages<'a>(
    package: &str,
    prefix: &str,
    descriptors: &'a [DescriptorProto],
    output: &mut Vec<(String, &'a DescriptorProto)>,
) {
    for descriptor in descriptors {
        let name = descriptor.name.as_deref().expect("message name is present");
        let local = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}.{name}")
        };
        output.push((format!("{package}.{local}"), descriptor));
        collect_messages(package, &local, &descriptor.nested_type, output);
    }
}

fn message_depth(message: &str, edges: &[(String, i64, String)], stack: &mut Vec<String>) -> u8 {
    assert!(
        !stack.iter().any(|active| active == message),
        "descriptor message graph must be acyclic"
    );
    stack.push(message.to_owned());
    let child_depth = edges
        .iter()
        .filter(|(source, _, _)| source == message)
        .map(|(_, _, target)| message_depth(target, edges, stack))
        .max()
        .unwrap_or(0);
    stack.pop();
    child_depth
        .checked_add(1)
        .expect("message depth is bounded")
}

fn rust_message_kind(full_name: &str) -> &'static str {
    match full_name {
        "mengxia.core.v1.ClientHello" => "ClientHello",
        "mengxia.core.v1.ServerHello" => "ServerHello",
        "mengxia.core.v1.ErrorEnvelope" => "ErrorEnvelope",
        "mengxia.core.v1.ErrorEnvelope.SafeDetailsEntry" => "SafeDetailsEntry",
        "mengxia.core.v1.HandshakeResponse" => "HandshakeResponse",
        "mengxia.core.v1.IngestAssetCopyRequest" => "IngestAssetCopyRequest",
        "mengxia.core.v1.IngestAssetCopyResult" => "IngestAssetCopyResult",
        "mengxia.core.v1.CoreRequest" => "CoreRequest",
        "mengxia.core.v1.CoreResponse" => "CoreResponse",
        _ => panic!("descriptor introduced an unreviewed message kind: {full_name}"),
    }
}

fn field<'a>(text: &'a str, prefix: &str) -> &'a str {
    let mut matches = text.lines().filter_map(|line| line.strip_prefix(prefix));
    let value = matches
        .next()
        .expect("proto provenance must contain every exact field");
    assert!(
        matches.next().is_none(),
        "proto provenance fields must be unique"
    );
    value
}

fn verify_digest(path: &Path, expected: &str) {
    assert!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "proto provenance digest must be lowercase SHA-256"
    );
    let bytes = fs::read(path).expect("committed proto input must be readable");
    let digest = Sha256::digest(bytes);
    let mut actual = String::with_capacity(64);
    for byte in digest {
        write!(&mut actual, "{byte:02x}").expect("writing to a String cannot fail");
    }
    assert_eq!(actual, expected, "committed proto input digest drifted");
}
