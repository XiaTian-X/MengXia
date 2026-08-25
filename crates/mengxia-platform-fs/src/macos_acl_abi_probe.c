#include "mengxia_acl_shim.h"

#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/acl.h>
#include <sys/mount.h>

_Static_assert(CHAR_BIT == 8, "MengXia requires 8-bit bytes");
_Static_assert(sizeof(int) == 4, "MengXia requires a 32-bit C int");
_Static_assert(__MAC_OS_X_VERSION_MAX_ALLOWED >= 101300,
               "MengXia requires macOS SDK 10.13 or newer");

_Static_assert(ACL_TYPE_EXTENDED == 0x100, "unexpected ACL_TYPE_EXTENDED");
_Static_assert(ACL_FIRST_ENTRY == 0, "unexpected ACL_FIRST_ENTRY");
_Static_assert(ACL_NEXT_ENTRY == -1, "unexpected ACL_NEXT_ENTRY");
_Static_assert(ACL_MAX_ENTRIES == 128, "unexpected ACL_MAX_ENTRIES");
_Static_assert(ACL_EXTENDED_ALLOW == 1, "unexpected ACL_EXTENDED_ALLOW");
_Static_assert(ACL_EXTENDED_DENY == 2, "unexpected ACL_EXTENDED_DENY");
_Static_assert(ACL_FLAG_DEFER_INHERIT == (1 << 0),
               "unexpected ACL_FLAG_DEFER_INHERIT");
_Static_assert(ACL_FLAG_NO_INHERIT == (1 << 17),
               "unexpected ACL_FLAG_NO_INHERIT");
_Static_assert(ACL_ENTRY_INHERITED == (1 << 4),
               "unexpected ACL_ENTRY_INHERITED");
_Static_assert(ACL_ENTRY_FILE_INHERIT == (1 << 5),
               "unexpected ACL_ENTRY_FILE_INHERIT");
_Static_assert(ACL_ENTRY_DIRECTORY_INHERIT == (1 << 6),
               "unexpected ACL_ENTRY_DIRECTORY_INHERIT");
_Static_assert(ACL_ENTRY_LIMIT_INHERIT == (1 << 7),
               "unexpected ACL_ENTRY_LIMIT_INHERIT");
_Static_assert(ACL_ENTRY_ONLY_INHERIT == (1 << 8),
               "unexpected ACL_ENTRY_ONLY_INHERIT");
_Static_assert(MENGXIA_ACL_EXTERNAL_MAX_V1 == 16384u,
               "unexpected external ACL bound");
_Static_assert(MNT_LOCAL == 0x00001000, "unexpected MNT_LOCAL");
_Static_assert(MNT_IGNORE_OWNERSHIP == 0x00200000,
               "unexpected MNT_IGNORE_OWNERSHIP");

_Static_assert(sizeof(struct mengxia_acl_summary_v1) == 40,
               "unexpected summary size");
_Static_assert(_Alignof(struct mengxia_acl_summary_v1) == 4,
               "unexpected summary alignment");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, abi_version) == 0,
               "unexpected abi_version offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, entry_count) == 4,
               "unexpected entry_count offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, allow_count) == 8,
               "unexpected allow_count offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, deny_count) == 12,
               "unexpected deny_count offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, acl_flags) == 16,
               "unexpected acl_flags offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, entry_flags_or) == 20,
               "unexpected entry_flags_or offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, inheritable_count) == 24,
               "unexpected inheritable_count offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, external_size) == 28,
               "unexpected external_size offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, os_errno) == 32,
               "unexpected os_errno offset");
_Static_assert(offsetof(struct mengxia_acl_summary_v1, reserved) == 36,
               "unexpected reserved offset");

typedef int32_t (*mengxia_acl_path_is_empty_signature_v1)(const char *,
                                                           int32_t *);
_Static_assert(
    _Generic(&mengxia_acl_path_is_empty_v1,
             mengxia_acl_path_is_empty_signature_v1: 1, default: 0),
    "unexpected path ACL probe signature");
