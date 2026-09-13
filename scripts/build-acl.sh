# Sourced helper; inspect only. ls -b escapes path bytes onto one header line.
build_acl_safe() (
    build_acl_listing=$(LC_ALL=C /bin/ls -ldben "$1" 2>&1) || exit 1
    [ "${#build_acl_listing}" -lt 16384 ] || exit 1
    printf '%s\n' "$build_acl_listing" | LC_ALL=C /usr/bin/awk -f "$repository_root/scripts/build-acl-policy.awk"
)
