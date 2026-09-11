#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

provenance=$repository_root/proto/core/v1/handshake.provenance

unverifiable() {
    echo "UNVERIFIABLE: $1" >&2
    exit 2
}

provenance_value() {
    key=$1
    count=$(/usr/bin/awk -F= -v key="$key" '$1 == key { count += 1 } END { print count + 0 }' "$provenance")
    [ "$count" -eq 1 ] || unverifiable "proto provenance key $key is missing or duplicated"
    /usr/bin/awk -F= -v key="$key" '$1 == key { sub(/^[^=]*=/, ""); print }' "$provenance"
}

[ -f "$provenance" ] || unverifiable "proto provenance is unavailable"
[ "$(provenance_value format)" = mengxia-proto-provenance-v1 ] || unverifiable "proto provenance format is unsupported"
version=$(provenance_value protoc_version)
artifact=$(provenance_value protoc_artifact)
artifact_sha256=$(provenance_value protoc_artifact_sha256)
descriptor_sha256=$(provenance_value descriptor_sha256)

case "$version" in
    *[!0-9.]*|.*|*..*|*.) unverifiable "protoc version is malformed" ;;
esac
[ "$artifact" = "protoc-$version-osx-aarch_64.zip" ] || unverifiable "protoc artifact name is not canonical"
case "$artifact_sha256:$descriptor_sha256" in
    *[!0-9a-f:]*) unverifiable "proto provenance digest is malformed" ;;
esac
[ "${#artifact_sha256}" -eq 64 ] && [ "${#descriptor_sha256}" -eq 64 ] \
    || unverifiable "proto provenance digest is malformed"

mkdir -p "$repository_root/target"
fixture=$(/usr/bin/mktemp -d "$repository_root/target/mengxia-proto-regeneration.XXXXXX")
cleanup() {
    /bin/rm -rf -- "$fixture"
}
trap cleanup EXIT HUP INT TERM

archive=$fixture/$artifact
if [ -n "${MENGXIA_PROTOC_ARCHIVE-}" ]; then
    [ -f "$MENGXIA_PROTOC_ARCHIVE" ] || unverifiable "explicit protoc archive is unavailable"
    /bin/cp "$MENGXIA_PROTOC_ARCHIVE" "$archive"
else
    url="https://github.com/protocolbuffers/protobuf/releases/download/v$version/$artifact"
    /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/curl --fail --location --silent --show-error \
        --output "$archive" "$url" || unverifiable "recorded protoc artifact could not be downloaded"
fi

actual_archive_sha256=$(/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/shasum -a 256 "$archive" | /usr/bin/awk '{print $1}')
[ "$actual_archive_sha256" = "$artifact_sha256" ] || unverifiable "recorded protoc artifact digest mismatched"

/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/unzip -q "$archive" -d "$fixture/tool" \
    || unverifiable "recorded protoc artifact could not be extracted"
compiler=$fixture/tool/bin/protoc
[ -x "$compiler" ] || unverifiable "recorded protoc compiler is unavailable"
[ "$(/usr/bin/env -i LC_ALL=C LANG=C "$compiler" --version)" = "libprotoc $version" ] \
    || unverifiable "recorded protoc compiler version mismatched"

/usr/bin/env -i LC_ALL=C LANG=C "$compiler" \
    --proto_path=$repository_root/proto/core/v1 \
    --descriptor_set_out=$fixture/handshake.pb \
    "$repository_root/proto/core/v1/handshake.proto" \
    || unverifiable "descriptor regeneration failed"

actual_descriptor_sha256=$(/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/shasum -a 256 "$fixture/handshake.pb" | /usr/bin/awk '{print $1}')
[ "$actual_descriptor_sha256" = "$descriptor_sha256" ] || {
    echo "PROTO_REGENERATION_MISMATCH expected=$descriptor_sha256 actual=$actual_descriptor_sha256" >&2
    exit 1
}
/usr/bin/cmp -s "$fixture/handshake.pb" "$repository_root/proto/core/v1/handshake.pb" || {
    echo "PROTO_REGENERATION_MISMATCH committed descriptor bytes differ" >&2
    exit 1
}

echo "PROTO_REGENERATION_OK protoc=$version descriptor_sha256=$actual_descriptor_sha256"
