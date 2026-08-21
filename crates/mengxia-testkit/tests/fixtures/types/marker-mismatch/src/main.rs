use mengxia_types::Id;

struct Asset;
struct Project;

fn requires_asset(_: Id<Asset>) {}

fn main() {
    let project = Id::<Project>::from_bytes([
        0x01, 0x89, 0x0f, 0x3e, 0x7a, 0x5b, 0x7c, 0x4d, 0x8e, 0x9f, 0x10, 0x29, 0x38, 0x47,
        0x56, 0xab,
    ])
    .expect("fixture uses valid UUIDv7 bytes");
    requires_asset(project);
}
