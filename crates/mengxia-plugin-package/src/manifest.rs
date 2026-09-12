use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::OnceLock;

use jsonschema::{Draft, Validator};
use mengxia_types::Sha256Digest;
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

use crate::dependency::{RuntimeDependencyDeclaration, RuntimeDependencyRole};

const MANIFEST_SCHEMA_ID: &str = "https://schemas.mengxia.local/plugin/manifest-v1.schema.json";
const MANIFEST_SCHEMA: &str = include_str!("../../../schemas/plugin/manifest-v1.schema.json");
const MAX_MANIFEST_BYTES: usize = 65_536;
const MAX_JSON_DEPTH: usize = 16;
const MAX_JSON_NODES: usize = 4_096;
const MAX_STRING_BYTES: usize = 4_096;
const MAX_CAPABILITIES: usize = 64;
const MAX_DEPENDENCIES: usize = 32;
const MAX_DEPENDENCY_BYTES: u64 = 4_294_967_296;

/// Closed, redaction-safe errors for the pure package inspection boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginPackageError {
    InputTooLarge,
    ResourceLimitExceeded,
    MalformedJson,
    DuplicateKey,
    ManifestInvalid,
    NoncanonicalBytes,
    InternalInvariant,
}

impl PluginPackageError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputTooLarge => "INPUT_TOO_LARGE",
            Self::ResourceLimitExceeded => "RESOURCE_LIMIT_EXCEEDED",
            Self::MalformedJson => "MALFORMED_JSON",
            Self::DuplicateKey => "DUPLICATE_KEY",
            Self::ManifestInvalid => "MANIFEST_INVALID",
            Self::NoncanonicalBytes => "NONCANONICAL_BYTES",
            Self::InternalInvariant => "INTERNAL_INVARIANT",
        }
    }
}

impl fmt::Display for PluginPackageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for PluginPackageError {}

/// A digest whose type identifies canonical Plugin package bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PackageDigest(Sha256Digest);

impl PackageDigest {
    #[must_use]
    pub const fn sha256(self) -> Sha256Digest {
        self.0
    }

    #[must_use]
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0.to_bytes()
    }
}

impl fmt::Display for PackageDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A canonical capability identifier.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId(Box<str>);

impl CapabilityId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A typed permission request. It is evidence only and never a grant.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PermissionRequest {
    kind: Box<str>,
    scope: Box<str>,
}

impl PermissionRequest {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn scope(&self) -> &str {
        &self.scope
    }

    #[must_use]
    pub fn canonical_identifier(&self) -> String {
        format!("{}/{}", self.kind, self.scope)
    }
}

/// Immutable result of successful, bounded manifest inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectedPluginPackage {
    canonical_bytes: Box<[u8]>,
    digest: PackageDigest,
    publisher: Box<str>,
    plugin_id: Box<str>,
    version: Box<str>,
    capabilities: Box<[CapabilityId]>,
    requested_permissions: Box<[PermissionRequest]>,
    runtime_dependencies: Box<[RuntimeDependencyDeclaration]>,
}

impl InspectedPluginPackage {
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    #[must_use]
    pub const fn digest(&self) -> PackageDigest {
        self.digest
    }

    #[must_use]
    pub fn publisher(&self) -> &str {
        &self.publisher
    }

    #[must_use]
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn capabilities(&self) -> &[CapabilityId] {
        &self.capabilities
    }

    #[must_use]
    pub fn requested_permissions(&self) -> &[PermissionRequest] {
        &self.requested_permissions
    }

    #[must_use]
    pub fn runtime_dependencies(&self) -> &[RuntimeDependencyDeclaration] {
        &self.runtime_dependencies
    }
}

/// Inspect one complete V1 manifest without filesystem, network or execution effects.
pub fn inspect_manifest(bytes: &[u8]) -> Result<InspectedPluginPackage, PluginPackageError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(PluginPackageError::InputTooLarge);
    }
    if bytes.is_empty() || bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(PluginPackageError::MalformedJson);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| PluginPackageError::MalformedJson)?;
    let parsed = Parser::new(text).parse()?;
    enforce_field_collection_caps(&parsed)?;
    let schema_value = parsed.to_serde();
    let validator = schema_validator()?;
    if !validator.is_valid(&schema_value) {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let fields = TypedManifest::from_json(parsed)?;
    let canonical = fields.canonical_bytes();
    if canonical.as_slice() != bytes {
        return Err(PluginPackageError::NoncanonicalBytes);
    }
    let digest = Sha256Digest::from_bytes(Sha256::digest(bytes).into());
    Ok(fields.into_inspected(canonical, PackageDigest(digest)))
}

fn enforce_field_collection_caps(value: &JsonValue) -> Result<(), PluginPackageError> {
    let JsonValue::Object(root) = value else {
        return Ok(());
    };
    for (field, limit) in [
        ("capabilities", MAX_CAPABILITIES),
        ("requested_permissions", 1),
        ("runtime_dependencies", MAX_DEPENDENCIES),
    ] {
        if matches!(root.get(field), Some(JsonValue::Array(values)) if values.len() > limit) {
            return Err(PluginPackageError::ResourceLimitExceeded);
        }
    }
    Ok(())
}

fn schema_validator() -> Result<&'static Validator, PluginPackageError> {
    static VALIDATOR: OnceLock<Result<Validator, ()>> = OnceLock::new();
    VALIDATOR
        .get_or_init(|| {
            let schema: Value = serde_json::from_str(MANIFEST_SCHEMA).map_err(|_| ())?;
            jsonschema::options()
                .with_draft(Draft::Draft202012)
                .offline()
                .should_validate_formats(false)
                .build(&schema)
                .map_err(|_| ())
        })
        .as_ref()
        .map_err(|()| PluginPackageError::InternalInvariant)
}

#[derive(Debug, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(u64),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl JsonValue {
    fn to_serde(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(*value),
            Self::Number(value) => Value::Number(Number::from(*value)),
            Self::String(value) => Value::String(value.clone()),
            Self::Array(values) => Value::Array(values.iter().map(Self::to_serde).collect()),
            Self::Object(values) => Value::Object(
                values
                    .iter()
                    .map(|(key, value)| (key.clone(), value.to_serde()))
                    .collect::<Map<_, _>>(),
            ),
        }
    }
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
    nodes: usize,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            nodes: 0,
        }
    }

    fn parse(mut self) -> Result<JsonValue, PluginPackageError> {
        self.skip_whitespace();
        let value = self.value(1, None)?;
        self.skip_whitespace();
        if self.offset != self.input.len() {
            return Err(PluginPackageError::MalformedJson);
        }
        Ok(value)
    }

    fn value(
        &mut self,
        depth: usize,
        array_limit: Option<usize>,
    ) -> Result<JsonValue, PluginPackageError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(PluginPackageError::ResourceLimitExceeded)?;
        if self.nodes > MAX_JSON_NODES {
            return Err(PluginPackageError::ResourceLimitExceeded);
        }
        match self.peek() {
            Some(b'{') => {
                if depth > MAX_JSON_DEPTH {
                    return Err(PluginPackageError::ResourceLimitExceeded);
                }
                self.object(depth)
            }
            Some(b'[') => {
                if depth > MAX_JSON_DEPTH {
                    return Err(PluginPackageError::ResourceLimitExceeded);
                }
                self.array(depth, array_limit)
            }
            Some(b'"') => self.string().map(JsonValue::String),
            Some(b't') => {
                self.literal("true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(JsonValue::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.number().map(JsonValue::Number),
            _ => Err(PluginPackageError::MalformedJson),
        }
    }

    fn object(&mut self, depth: usize) -> Result<JsonValue, PluginPackageError> {
        self.consume(b'{')?;
        self.skip_whitespace();
        let mut values = BTreeMap::new();
        if self.take(b'}') {
            return Ok(JsonValue::Object(values));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(PluginPackageError::MalformedJson);
            }
            let key = self.string()?;
            if values.contains_key(&key) {
                return Err(PluginPackageError::DuplicateKey);
            }
            self.skip_whitespace();
            self.consume(b':')?;
            self.skip_whitespace();
            let limit = if depth == 1 {
                field_array_limit(&key)
            } else {
                None
            };
            let value = self.value(depth.saturating_add(1), limit)?;
            values.insert(key, value);
            self.skip_whitespace();
            if self.take(b'}') {
                break;
            }
            self.consume(b',')?;
            self.skip_whitespace();
        }
        Ok(JsonValue::Object(values))
    }

    fn array(
        &mut self,
        depth: usize,
        limit: Option<usize>,
    ) -> Result<JsonValue, PluginPackageError> {
        self.consume(b'[')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.take(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            if limit.is_some_and(|limit| values.len() >= limit) {
                return Err(PluginPackageError::ResourceLimitExceeded);
            }
            let value = self.value(depth.saturating_add(1), None)?;
            values.push(value);
            self.skip_whitespace();
            if self.take(b']') {
                break;
            }
            self.consume(b',')?;
            self.skip_whitespace();
        }
        Ok(JsonValue::Array(values))
    }

    fn string(&mut self) -> Result<String, PluginPackageError> {
        self.consume(b'"')?;
        let mut output = String::new();
        loop {
            let byte = self.peek().ok_or(PluginPackageError::MalformedJson)?;
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.offset += 1;
                    let escaped = self.peek().ok_or(PluginPackageError::MalformedJson)?;
                    self.offset += 1;
                    match escaped {
                        b'"' => output.push('"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => self.push_unicode_escape(&mut output)?,
                        _ => return Err(PluginPackageError::MalformedJson),
                    }
                }
                0x00..=0x1f => return Err(PluginPackageError::MalformedJson),
                _ if byte.is_ascii() => {
                    self.offset += 1;
                    output.push(char::from(byte));
                }
                _ => {
                    let ch = self.input[self.offset..]
                        .chars()
                        .next()
                        .ok_or(PluginPackageError::MalformedJson)?;
                    self.offset += ch.len_utf8();
                    output.push(ch);
                }
            }
            if output.len() > MAX_STRING_BYTES {
                return Err(PluginPackageError::ResourceLimitExceeded);
            }
        }
    }

    fn push_unicode_escape(&mut self, output: &mut String) -> Result<(), PluginPackageError> {
        let first = self.hex_quad()?;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            if !self.take(b'\\') || !self.take(b'u') {
                return Err(PluginPackageError::MalformedJson);
            }
            let second = self.hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(PluginPackageError::MalformedJson);
            }
            0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(PluginPackageError::MalformedJson);
        } else {
            u32::from(first)
        };
        output.push(char::from_u32(scalar).ok_or(PluginPackageError::MalformedJson)?);
        Ok(())
    }

    fn hex_quad(&mut self) -> Result<u16, PluginPackageError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let byte = self.peek().ok_or(PluginPackageError::MalformedJson)?;
            self.offset += 1;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a' + 10),
                b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => return Err(PluginPackageError::MalformedJson),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<u64, PluginPackageError> {
        let start = self.offset;
        let negative = self.take(b'-');
        match self.peek() {
            Some(b'0') => self.offset += 1,
            Some(b'1'..=b'9') => {
                self.offset += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.offset += 1;
                }
            }
            _ => return Err(PluginPackageError::MalformedJson),
        }
        let fraction = if self.take(b'.') {
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(PluginPackageError::MalformedJson);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
            true
        } else {
            false
        };
        let exponent = if matches!(self.peek(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(PluginPackageError::MalformedJson);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.offset += 1;
            }
            true
        } else {
            false
        };
        if !matches!(
            self.peek(),
            None | Some(b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}')
        ) {
            return Err(PluginPackageError::MalformedJson);
        }
        if negative || fraction || exponent {
            return Err(PluginPackageError::ManifestInvalid);
        }
        self.input[start..self.offset]
            .parse()
            .map_err(|_| PluginPackageError::ManifestInvalid)
    }

    fn literal(&mut self, literal: &str) -> Result<(), PluginPackageError> {
        if self.input[self.offset..].starts_with(literal) {
            self.offset += literal.len();
            if matches!(
                self.peek(),
                None | Some(b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}')
            ) {
                return Ok(());
            }
        }
        Err(PluginPackageError::MalformedJson)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.offset += 1;
        }
    }

    fn consume(&mut self, expected: u8) -> Result<(), PluginPackageError> {
        if self.take(expected) {
            Ok(())
        } else {
            Err(PluginPackageError::MalformedJson)
        }
    }

    fn take(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.offset).copied()
    }
}

fn field_array_limit(field: &str) -> Option<usize> {
    match field {
        "capabilities" => Some(MAX_CAPABILITIES),
        "requested_permissions" => Some(1),
        "runtime_dependencies" => Some(MAX_DEPENDENCIES),
        _ => None,
    }
}

struct TypedManifest {
    publisher: Box<str>,
    plugin_id: Box<str>,
    version: Box<str>,
    capabilities: Vec<CapabilityId>,
    requested_permissions: Vec<PermissionRequest>,
    runtime_dependencies: Vec<RuntimeDependencyDeclaration>,
}

impl TypedManifest {
    fn from_json(value: JsonValue) -> Result<Self, PluginPackageError> {
        let JsonValue::Object(mut root) = value else {
            return Err(PluginPackageError::ManifestInvalid);
        };
        if root.len() != 8 {
            return Err(PluginPackageError::ManifestInvalid);
        }
        if require_string(root.remove("$schema"))? != MANIFEST_SCHEMA_ID
            || require_number(root.remove("manifest_version"))? != 1
        {
            return Err(PluginPackageError::ManifestInvalid);
        }
        let publisher = require_string(root.remove("publisher"))?;
        if !valid_ascii_token(&publisher, 1, 128, |byte, _| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
        }) {
            return Err(PluginPackageError::ManifestInvalid);
        }
        let plugin_id = require_string(root.remove("plugin_id"))?;
        if !valid_ascii_token(&plugin_id, 1, 128, |byte, first| {
            byte.is_ascii_lowercase()
                || (!first && (byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')))
        }) {
            return Err(PluginPackageError::ManifestInvalid);
        }
        let version = require_string(root.remove("version"))?;
        if version.is_empty() || version.len() > 64 || !version.is_ascii() {
            return Err(PluginPackageError::ManifestInvalid);
        }
        let parsed_version =
            semver::Version::parse(&version).map_err(|_| PluginPackageError::ManifestInvalid)?;
        if !parsed_version.build.is_empty() || parsed_version.to_string() != version {
            return Err(PluginPackageError::ManifestInvalid);
        }

        let mut capabilities = require_array(root.remove("capabilities"))?
            .into_iter()
            .map(|value| require_string(Some(value)).and_then(parse_capability))
            .collect::<Result<Vec<_>, _>>()?;
        if capabilities.len() > MAX_CAPABILITIES {
            return Err(PluginPackageError::ResourceLimitExceeded);
        }
        capabilities.sort();
        if has_duplicates(&capabilities) {
            return Err(PluginPackageError::ManifestInvalid);
        }

        let permissions_json = require_array(root.remove("requested_permissions"))?;
        if permissions_json.len() > 1 {
            return Err(PluginPackageError::ResourceLimitExceeded);
        }
        let mut requested_permissions = permissions_json
            .into_iter()
            .map(parse_permission)
            .collect::<Result<Vec<_>, _>>()?;
        requested_permissions.sort();
        if has_duplicates(&requested_permissions) {
            return Err(PluginPackageError::ManifestInvalid);
        }

        let dependencies_json = require_array(root.remove("runtime_dependencies"))?;
        if dependencies_json.len() > MAX_DEPENDENCIES {
            return Err(PluginPackageError::ResourceLimitExceeded);
        }
        let mut aggregate = 0_u64;
        let mut runtime_dependencies = dependencies_json
            .into_iter()
            .map(|value| {
                let dependency = parse_dependency(value)?;
                aggregate = aggregate
                    .checked_add(dependency.byte_length())
                    .ok_or(PluginPackageError::ResourceLimitExceeded)?;
                if aggregate > MAX_DEPENDENCY_BYTES {
                    return Err(PluginPackageError::ResourceLimitExceeded);
                }
                Ok(dependency)
            })
            .collect::<Result<Vec<_>, _>>()?;
        runtime_dependencies.sort_by(|left, right| left.dependency_id().cmp(right.dependency_id()));
        if runtime_dependencies.is_empty()
            || runtime_dependencies
                .windows(2)
                .any(|pair| pair[0].dependency_id() == pair[1].dependency_id())
            || runtime_dependencies
                .iter()
                .filter(|dependency| dependency.role() == RuntimeDependencyRole::PluginEntrypoint)
                .count()
                != 1
        {
            return Err(PluginPackageError::ManifestInvalid);
        }
        if !root.is_empty() {
            return Err(PluginPackageError::ManifestInvalid);
        }
        Ok(Self {
            publisher: publisher.into_boxed_str(),
            plugin_id: plugin_id.into_boxed_str(),
            version: version.into_boxed_str(),
            capabilities,
            requested_permissions,
            runtime_dependencies,
        })
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = String::new();
        output.push_str("{\"$schema\":");
        push_json_string(&mut output, MANIFEST_SCHEMA_ID);
        output.push_str(",\"capabilities\":[");
        for (index, capability) in self.capabilities.iter().enumerate() {
            push_separator(&mut output, index);
            push_json_string(&mut output, capability.as_str());
        }
        output.push_str("],\"manifest_version\":1,\"plugin_id\":");
        push_json_string(&mut output, &self.plugin_id);
        output.push_str(",\"publisher\":");
        push_json_string(&mut output, &self.publisher);
        output.push_str(",\"requested_permissions\":[");
        for (index, permission) in self.requested_permissions.iter().enumerate() {
            push_separator(&mut output, index);
            output.push_str("{\"kind\":");
            push_json_string(&mut output, permission.kind());
            output.push_str(",\"scope\":");
            push_json_string(&mut output, permission.scope());
            output.push('}');
        }
        output.push_str("],\"runtime_dependencies\":[");
        for (index, dependency) in self.runtime_dependencies.iter().enumerate() {
            push_separator(&mut output, index);
            output.push_str("{\"byte_length\":");
            output.push_str(&dependency.byte_length().to_string());
            output.push_str(",\"dependency_id\":");
            push_json_string(&mut output, dependency.dependency_id());
            output.push_str(",\"role\":");
            push_json_string(&mut output, dependency.role().as_manifest_str());
            output.push_str(",\"sha256\":");
            push_json_string(&mut output, &dependency.sha256().to_string());
            output.push_str(",\"target\":");
            push_json_string(&mut output, dependency.target());
            output.push('}');
        }
        output.push_str("],\"version\":");
        push_json_string(&mut output, &self.version);
        output.push('}');
        output.into_bytes()
    }

    fn into_inspected(
        self,
        canonical_bytes: Vec<u8>,
        digest: PackageDigest,
    ) -> InspectedPluginPackage {
        InspectedPluginPackage {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            digest,
            publisher: self.publisher,
            plugin_id: self.plugin_id,
            version: self.version,
            capabilities: self.capabilities.into_boxed_slice(),
            requested_permissions: self.requested_permissions.into_boxed_slice(),
            runtime_dependencies: self.runtime_dependencies.into_boxed_slice(),
        }
    }
}

fn require_string(value: Option<JsonValue>) -> Result<String, PluginPackageError> {
    match value {
        Some(JsonValue::String(value)) => Ok(value),
        _ => Err(PluginPackageError::ManifestInvalid),
    }
}

fn require_number(value: Option<JsonValue>) -> Result<u64, PluginPackageError> {
    match value {
        Some(JsonValue::Number(value)) => Ok(value),
        _ => Err(PluginPackageError::ManifestInvalid),
    }
}

fn require_array(value: Option<JsonValue>) -> Result<Vec<JsonValue>, PluginPackageError> {
    match value {
        Some(JsonValue::Array(value)) => Ok(value),
        _ => Err(PluginPackageError::ManifestInvalid),
    }
}

fn parse_capability(value: String) -> Result<CapabilityId, PluginPackageError> {
    if value.len() < 5 || value.len() > 128 || !value.is_ascii() {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let (name, version) = value
        .rsplit_once('@')
        .ok_or(PluginPackageError::ManifestInvalid)?;
    let segments = name.split('.').collect::<Vec<_>>();
    if !(2..=8).contains(&segments.len())
        || segments.iter().any(|segment| {
            !valid_ascii_token(segment, 1, 32, |byte, first| {
                byte.is_ascii_lowercase() || (!first && (byte.is_ascii_digit() || byte == b'_'))
            })
        })
        || version.is_empty()
        || version.starts_with('0')
        || version.parse::<u32>().is_err()
    {
        return Err(PluginPackageError::ManifestInvalid);
    }
    Ok(CapabilityId(value.into_boxed_str()))
}

fn parse_permission(value: JsonValue) -> Result<PermissionRequest, PluginPackageError> {
    let JsonValue::Object(mut object) = value else {
        return Err(PluginPackageError::ManifestInvalid);
    };
    if object.len() != 2 {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let kind = require_string(object.remove("kind"))?;
    let scope = require_string(object.remove("scope"))?;
    if !object.is_empty() || kind != "broker.asset.read@1" || scope != "run-inputs" {
        return Err(PluginPackageError::ManifestInvalid);
    }
    Ok(PermissionRequest {
        kind: kind.into_boxed_str(),
        scope: scope.into_boxed_str(),
    })
}

fn parse_dependency(value: JsonValue) -> Result<RuntimeDependencyDeclaration, PluginPackageError> {
    let JsonValue::Object(mut object) = value else {
        return Err(PluginPackageError::ManifestInvalid);
    };
    if object.len() != 5 {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let dependency_id = require_string(object.remove("dependency_id"))?;
    if !valid_ascii_token(&dependency_id, 1, 64, |byte, first| {
        byte.is_ascii_lowercase()
            || (!first && (byte.is_ascii_digit() || matches!(byte, b'_' | b'-')))
    }) {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let role = match require_string(object.remove("role"))?.as_str() {
        "PLUGIN_ENTRYPOINT" => RuntimeDependencyRole::PluginEntrypoint,
        "TOOL" => RuntimeDependencyRole::Tool,
        _ => return Err(PluginPackageError::ManifestInvalid),
    };
    let target = require_string(object.remove("target"))?;
    if target != "aarch64-apple-darwin" {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let byte_length = require_number(object.remove("byte_length"))?;
    if !(1..=1_073_741_824).contains(&byte_length) {
        return Err(PluginPackageError::ManifestInvalid);
    }
    let digest_text = require_string(object.remove("sha256"))?;
    let sha256 =
        Sha256Digest::from_str(&digest_text).map_err(|_| PluginPackageError::ManifestInvalid)?;
    if sha256.to_bytes() == [0; 32] || !object.is_empty() {
        return Err(PluginPackageError::ManifestInvalid);
    }
    Ok(RuntimeDependencyDeclaration {
        dependency_id: dependency_id.into_boxed_str(),
        role,
        target: target.into_boxed_str(),
        byte_length,
        sha256,
    })
}

fn valid_ascii_token(
    value: &str,
    minimum: usize,
    maximum: usize,
    allowed: impl Fn(u8, bool) -> bool,
) -> bool {
    value.is_ascii()
        && (minimum..=maximum).contains(&value.len())
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| allowed(byte, index == 0))
}

fn has_duplicates<T: PartialEq>(values: &[T]) -> bool {
    values.windows(2).any(|pair| pair[0] == pair[1])
}

fn push_separator(output: &mut String, index: usize) {
    if index != 0 {
        output.push(',');
    }
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0008}' => output.push_str("\\b"),
            '\u{000c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{0000}'..='\u{001f}' => {
                use fmt::Write;
                let _ = write!(output, "\\u{:04x}", u32::from(ch));
            }
            _ => output.push(ch),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_FILE: &[u8] =
        include_bytes!("../../mengxia-testkit/tests/fixtures/task_010/manifest-v1.golden.json");

    fn golden() -> &'static [u8] {
        GOLDEN_FILE
    }

    #[test]
    fn golden_manifest_is_canonical_typed_and_digest_bound() {
        let package = inspect_manifest(golden()).expect("golden manifest");
        assert_eq!(package.canonical_bytes(), golden());
        assert_eq!(package.publisher(), "example-untrusted-label");
        assert_eq!(package.plugin_id(), "example.transcoder");
        assert_eq!(package.version(), "1.0.0");
        assert_eq!(package.capabilities().len(), 1);
        assert_eq!(package.requested_permissions().len(), 1);
        assert_eq!(package.runtime_dependencies().len(), 1);
        assert_eq!(
            package.digest().to_bytes(),
            <[u8; 32]>::from(Sha256::digest(golden()))
        );
    }

    #[test]
    fn malformed_and_non_v1_numbers_have_exact_classes() {
        for token in ["+1", "01", ".1", "NaN", "Infinity", "1e", "1e+"] {
            assert_eq!(
                inspect_manifest(token.as_bytes()),
                Err(PluginPackageError::MalformedJson),
                "{token}"
            );
        }
        for token in ["-1", "-0", "1.0", "1e0", "1e999", "18446744073709551616"] {
            assert_eq!(
                inspect_manifest(token.as_bytes()),
                Err(PluginPackageError::ManifestInvalid),
                "{token}"
            );
        }
    }

    #[test]
    fn duplicate_cap_and_noncanonical_inputs_fail_closed() {
        assert_eq!(
            inspect_manifest(br#"{"x":1,"x":2}"#),
            Err(PluginPackageError::DuplicateKey)
        );
        assert_eq!(
            inspect_manifest(br#"{"plugin_id":1,"plugin_\u0069d":2}"#),
            Err(PluginPackageError::DuplicateKey)
        );
        for invalid_escape in [r#""\uD800""#, r#""\uDC00""#, r#""\uD800\u0041""#] {
            assert_eq!(
                inspect_manifest(invalid_escape.as_bytes()),
                Err(PluginPackageError::MalformedJson)
            );
        }
        let mut padded = b" ".to_vec();
        padded.extend_from_slice(golden());
        assert_eq!(
            inspect_manifest(&padded),
            Err(PluginPackageError::NoncanonicalBytes)
        );
        let oversized = vec![b' '; MAX_MANIFEST_BYTES + 1];
        assert_eq!(
            inspect_manifest(&oversized),
            Err(PluginPackageError::InputTooLarge)
        );
    }

    #[test]
    fn schema_builder_is_explicitly_offline_and_rejects_external_refs() {
        let schema = serde_json::json!({"$ref": "https://invalid.mengxia.test/schema"});
        let validator = jsonschema::options()
            .with_draft(Draft::Draft202012)
            .offline()
            .should_validate_formats(false)
            .build(&schema);
        assert!(validator.is_err());
    }

    #[test]
    fn parser_depth_node_and_decoded_string_caps_are_exact() {
        let depth_15 = format!("{}0{}", "[".repeat(15), "]".repeat(15));
        assert!(Parser::new(&depth_15).parse().is_ok());
        let depth_16 = format!("{}0{}", "[".repeat(16), "]".repeat(16));
        assert!(Parser::new(&depth_16).parse().is_ok());
        let depth_17 = format!("{}0{}", "[".repeat(17), "]".repeat(17));
        assert_eq!(
            Parser::new(&depth_17).parse(),
            Err(PluginPackageError::ResourceLimitExceeded)
        );
        let nodes_4095 = format!("[{}]", vec!["0"; MAX_JSON_NODES - 2].join(","));
        assert!(Parser::new(&nodes_4095).parse().is_ok());
        let nodes_4096 = format!("[{}]", vec!["0"; MAX_JSON_NODES - 1].join(","));
        assert!(Parser::new(&nodes_4096).parse().is_ok());
        let nodes_4097 = format!("[{}]", vec!["0"; MAX_JSON_NODES].join(","));
        assert_eq!(
            Parser::new(&nodes_4097).parse(),
            Err(PluginPackageError::ResourceLimitExceeded)
        );
        let string_4095 = format!("\"{}\"", "a".repeat(MAX_STRING_BYTES - 1));
        assert!(Parser::new(&string_4095).parse().is_ok());
        let string_4096 = format!("\"{}\"", "a".repeat(MAX_STRING_BYTES));
        assert!(Parser::new(&string_4096).parse().is_ok());
        let string_4097 = format!("\"{}\"", "a".repeat(MAX_STRING_BYTES + 1));
        assert_eq!(
            Parser::new(&string_4097).parse(),
            Err(PluginPackageError::ResourceLimitExceeded)
        );
    }
}
