#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

provenance=$repository_root/proto/core/v1/handshake.provenance
proto=$repository_root/proto/core/v1/handshake.proto

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
[ -f "$proto" ] || unverifiable "proto source is unavailable"
[ "$(/usr/bin/stat -f %z "$provenance")" -le 8192 ] \
    || unverifiable "proto provenance is oversized"
/usr/bin/awk -F= '
    BEGIN {
        expected = "|format|proto_sha256|descriptor_sha256|protoc_version|protoc_artifact|protoc_artifact_sha256|prost_build_version|"
    }
    {
        if (NF != 2 || $1 == "" || $2 == "" || index(expected, "|" $1 "|") == 0 || seen[$1]++) exit 1
        count++
    }
    END { if (count != 7) exit 1 }
' "$provenance" || unverifiable "proto provenance schema is malformed"
[ "$(provenance_value format)" = mengxia-proto-provenance-v1 ] || unverifiable "proto provenance format is unsupported"
proto_sha256=$(provenance_value proto_sha256)
version=$(provenance_value protoc_version)
artifact=$(provenance_value protoc_artifact)
artifact_sha256=$(provenance_value protoc_artifact_sha256)
descriptor_sha256=$(provenance_value descriptor_sha256)
prost_build_version=$(provenance_value prost_build_version)

case "$version" in
    *[!0-9.]*|.*|*..*|*.) unverifiable "protoc version is malformed" ;;
esac
[ -n "$version" ] || unverifiable "protoc version is malformed"
case "$prost_build_version" in
    *[!0-9.]*|.*|*..*|*.) unverifiable "prost-build version is malformed" ;;
esac
[ -n "$prost_build_version" ] || unverifiable "prost-build version is malformed"
[ "$artifact" = "protoc-$version-osx-aarch_64.zip" ] || unverifiable "protoc artifact name is not canonical"
case "$proto_sha256:$artifact_sha256:$descriptor_sha256" in
    *[!0-9a-f:]*) unverifiable "proto provenance digest is malformed" ;;
esac
[ "${#proto_sha256}" -eq 64 ] && [ "${#artifact_sha256}" -eq 64 ] \
    && [ "${#descriptor_sha256}" -eq 64 ] \
    || unverifiable "proto provenance digest is malformed"

actual_proto_sha256=$(/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/shasum -a 256 "$proto" | /usr/bin/awk '{print $1}')
[ "$actual_proto_sha256" = "$proto_sha256" ] \
    || unverifiable "proto source digest mismatched"

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
    "$proto" \
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

echo "PROTO_REGENERATION_OK protoc=$version prost_build=$prost_build_version descriptor_sha256=$actual_descriptor_sha256"
