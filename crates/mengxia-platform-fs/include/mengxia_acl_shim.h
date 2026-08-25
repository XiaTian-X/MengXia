#ifndef MENGXIA_ACL_SHIM_H
#define MENGXIA_ACL_SHIM_H

#include <stdint.h>

#define MENGXIA_ACL_ABI_V1 1u
#define MENGXIA_ACL_EXTERNAL_MAX_V1 16384u

#if defined(__GNUC__)
#define MENGXIA_ACL_EXPORT __attribute__((visibility("default")))
#else
#error "the MengXia ACL shim requires an Apple clang-compatible compiler"
#endif

struct mengxia_acl_summary_v1 {
    uint32_t abi_version;
    uint32_t entry_count;
    uint32_t allow_count;
    uint32_t deny_count;
    uint32_t acl_flags;
    uint32_t entry_flags_or;
    uint32_t inheritable_count;
    uint32_t external_size;
    int32_t os_errno;
    uint32_t reserved;
};

MENGXIA_ACL_EXPORT uint32_t mengxia_acl_abi_version_v1(void);
MENGXIA_ACL_EXPORT int32_t mengxia_acl_inspect_fd_v1(
    int32_t fd, struct mengxia_acl_summary_v1 *out);
MENGXIA_ACL_EXPORT int32_t mengxia_acl_path_is_empty_v1(
    const char *path, int32_t *os_errno);

#endif
