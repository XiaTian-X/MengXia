#include "mengxia_acl_shim.h"

#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/acl.h>

enum mengxia_acl_status_v1 {
    MENGXIA_ACL_OK = 0,
    MENGXIA_ACL_INVALID_ARGUMENT = 1,
    MENGXIA_ACL_OS_ERROR = 2,
    MENGXIA_ACL_MALFORMED_ITERATION = 3,
    MENGXIA_ACL_UNKNOWN_TAG = 4,
    MENGXIA_ACL_UNKNOWN_SDK_RESULT = 5,
    MENGXIA_ACL_ENTRY_LIMIT_EXCEEDED = 6,
    MENGXIA_ACL_UNKNOWN_FLAG_BITS = 7,
};

enum normalized_acl_flags_v1 {
    MENGXIA_ACL_DEFER_INHERIT = 1u << 0,
    MENGXIA_ACL_NO_INHERIT = 1u << 1,
};

enum normalized_entry_flags_v1 {
    MENGXIA_ENTRY_INHERITED = 1u << 0,
    MENGXIA_ENTRY_FILE_INHERIT = 1u << 1,
    MENGXIA_ENTRY_DIRECTORY_INHERIT = 1u << 2,
    MENGXIA_ENTRY_LIMIT_INHERIT = 1u << 3,
    MENGXIA_ENTRY_ONLY_INHERIT = 1u << 4,
};

struct mengxia_acl_backend_v1 {
    acl_t (*get_fd)(int, acl_type_t);
    int (*get_entry)(acl_t, int, acl_entry_t *);
    int (*get_tag_type)(acl_entry_t, acl_tag_t *);
    int (*get_flagset)(void *, acl_flagset_t *);
    int (*get_flag)(acl_flagset_t, acl_flag_t);
    int (*clear_flags)(acl_flagset_t);
    int (*add_flag)(acl_flagset_t, acl_flag_t);
    acl_t (*duplicate)(acl_t);
    int (*validate)(acl_t);
    ssize_t (*size)(acl_t);
    ssize_t (*copy_external)(void *, acl_t, ssize_t);
    int (*release_acl)(void *);
    void *(*allocate)(size_t);
    void (*deallocate)(void *);
};

__attribute__((visibility("hidden"))) int32_t
mengxia_acl_inspect_fd_core_v1(
    int32_t fd, struct mengxia_acl_summary_v1 *out,
    const struct mengxia_acl_backend_v1 *backend);

static int32_t status_for_errno(struct mengxia_acl_summary_v1 *out) {
    int captured = errno;
    out->os_errno = captured < 0 ? 0 : captured;
    return MENGXIA_ACL_OS_ERROR;
}

static int query_flag(const struct mengxia_acl_backend_v1 *backend,
                      acl_flagset_t set, acl_flag_t flag, uint32_t normalized,
                      uint32_t *word) {
    int result = backend->get_flag(set, flag);
    if (result == 1) {
        *word |= normalized;
        return MENGXIA_ACL_OK;
    }
    if (result == 0) {
        return MENGXIA_ACL_OK;
    }
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
}

static int read_acl_flags(const struct mengxia_acl_backend_v1 *backend,
                          void *object, uint32_t *word) {
    acl_flagset_t set = NULL;
    int result = backend->get_flagset(object, &set);
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    if (result != 0 || set == NULL) {
        return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
    }

    int status = query_flag(backend, set, ACL_FLAG_DEFER_INHERIT,
                            MENGXIA_ACL_DEFER_INHERIT, word);
    if (status != MENGXIA_ACL_OK) {
        return status;
    }
    return query_flag(backend, set, ACL_FLAG_NO_INHERIT, MENGXIA_ACL_NO_INHERIT,
                      word);
}

static int read_entry_flags(const struct mengxia_acl_backend_v1 *backend,
                            acl_entry_t entry, uint32_t *word) {
    acl_flagset_t set = NULL;
    int result = backend->get_flagset(entry, &set);
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    if (result != 0 || set == NULL) {
        return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
    }

    const acl_flag_t sdk_flags[] = {
        ACL_ENTRY_INHERITED,
        ACL_ENTRY_FILE_INHERIT,
        ACL_ENTRY_DIRECTORY_INHERIT,
        ACL_ENTRY_LIMIT_INHERIT,
        ACL_ENTRY_ONLY_INHERIT,
    };
    const uint32_t normalized_flags[] = {
        MENGXIA_ENTRY_INHERITED,
        MENGXIA_ENTRY_FILE_INHERIT,
        MENGXIA_ENTRY_DIRECTORY_INHERIT,
        MENGXIA_ENTRY_LIMIT_INHERIT,
        MENGXIA_ENTRY_ONLY_INHERIT,
    };
    for (size_t index = 0; index < 5; ++index) {
        int status = query_flag(backend, set, sdk_flags[index],
                                normalized_flags[index], word);
        if (status != MENGXIA_ACL_OK) {
            return status;
        }
    }
    return MENGXIA_ACL_OK;
}

static int restore_acl_flags(const struct mengxia_acl_backend_v1 *backend,
                             void *object, uint32_t word) {
    acl_flagset_t set = NULL;
    int result = backend->get_flagset(object, &set);
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    if (result != 0 || set == NULL) {
        return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
    }
    result = backend->clear_flags(set);
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    if (result != 0) {
        return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
    }
    if ((word & MENGXIA_ACL_DEFER_INHERIT) != 0) {
        result = backend->add_flag(set, ACL_FLAG_DEFER_INHERIT);
        if (result == -1) {
            return MENGXIA_ACL_OS_ERROR;
        }
        if (result != 0) {
            return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        }
    }
    if ((word & MENGXIA_ACL_NO_INHERIT) != 0) {
        result = backend->add_flag(set, ACL_FLAG_NO_INHERIT);
        if (result == -1) {
            return MENGXIA_ACL_OS_ERROR;
        }
        if (result != 0) {
            return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        }
    }
    return MENGXIA_ACL_OK;
}

static int restore_entry_flags(const struct mengxia_acl_backend_v1 *backend,
                               acl_entry_t entry, uint32_t word) {
    acl_flagset_t set = NULL;
    int result = backend->get_flagset(entry, &set);
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    if (result != 0 || set == NULL) {
        return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
    }
    result = backend->clear_flags(set);
    if (result == -1) {
        return MENGXIA_ACL_OS_ERROR;
    }
    if (result != 0) {
        return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
    }

    const uint32_t normalized_flags[] = {
        MENGXIA_ENTRY_INHERITED,
        MENGXIA_ENTRY_FILE_INHERIT,
        MENGXIA_ENTRY_DIRECTORY_INHERIT,
        MENGXIA_ENTRY_LIMIT_INHERIT,
        MENGXIA_ENTRY_ONLY_INHERIT,
    };
    const acl_flag_t sdk_flags[] = {
        ACL_ENTRY_INHERITED,
        ACL_ENTRY_FILE_INHERIT,
        ACL_ENTRY_DIRECTORY_INHERIT,
        ACL_ENTRY_LIMIT_INHERIT,
        ACL_ENTRY_ONLY_INHERIT,
    };
    for (size_t index = 0; index < 5; ++index) {
        if ((word & normalized_flags[index]) != 0) {
            result = backend->add_flag(set, sdk_flags[index]);
            if (result == -1) {
                return MENGXIA_ACL_OS_ERROR;
            }
            if (result != 0) {
                return MENGXIA_ACL_UNKNOWN_SDK_RESULT;
            }
        }
    }
    return MENGXIA_ACL_OK;
}

uint32_t mengxia_acl_abi_version_v1(void) { return MENGXIA_ACL_ABI_V1; }

static const struct mengxia_acl_backend_v1 real_backend_v1 = {
    .get_fd = acl_get_fd_np,
    .get_entry = acl_get_entry,
    .get_tag_type = acl_get_tag_type,
    .get_flagset = acl_get_flagset_np,
    .get_flag = acl_get_flag_np,
    .clear_flags = acl_clear_flags_np,
    .add_flag = acl_add_flag_np,
    .duplicate = acl_dup,
    .validate = acl_valid,
    .size = acl_size,
    .copy_external = acl_copy_ext,
    .release_acl = acl_free,
    .allocate = malloc,
    .deallocate = free,
};

int32_t mengxia_acl_inspect_fd_v1(int32_t fd,
                                  struct mengxia_acl_summary_v1 *out) {
    return mengxia_acl_inspect_fd_core_v1(fd, out, &real_backend_v1);
}

int32_t mengxia_acl_path_is_empty_v1(const char *path, int32_t *os_errno) {
    if (path == NULL || os_errno == NULL) {
        return MENGXIA_ACL_INVALID_ARGUMENT;
    }
    *os_errno = 0;
    errno = 0;
    acl_t acl = acl_get_link_np(path, ACL_TYPE_EXTENDED);
    if (acl == NULL) {
        if (errno == ENOENT) {
            return MENGXIA_ACL_OK;
        }
        *os_errno = errno < 0 ? 0 : errno;
        return MENGXIA_ACL_OS_ERROR;
    }
    if (acl_free(acl) != 0) {
        *os_errno = errno < 0 ? 0 : errno;
        return MENGXIA_ACL_OS_ERROR;
    }
    return MENGXIA_ACL_UNKNOWN_FLAG_BITS;
}

int32_t mengxia_acl_inspect_fd_core_v1(
    int32_t fd, struct mengxia_acl_summary_v1 *out,
    const struct mengxia_acl_backend_v1 *backend) {
    if (out == NULL || fd < 0 || backend == NULL) {
        return MENGXIA_ACL_INVALID_ARGUMENT;
    }
    memset(out, 0, sizeof(*out));
    out->abi_version = MENGXIA_ACL_ABI_V1;

    int32_t status = MENGXIA_ACL_OK;
    acl_t original = NULL;
    acl_t duplicate = NULL;
    unsigned char *original_bytes = NULL;
    unsigned char *duplicate_bytes = NULL;
    uint32_t entry_flags[ACL_MAX_ENTRIES] = {0};

    errno = 0;
    original = backend->get_fd(fd, ACL_TYPE_EXTENDED);
    if (original == NULL) {
        /* On macOS, an existing fd with no extended ACL is reported as
         * ENOENT. This is the exact empty-ACL state, not a pathname lookup. */
        if (errno == ENOENT) {
            out->external_size = 0;
            status = MENGXIA_ACL_OK;
            goto cleanup;
        }
        status = status_for_errno(out);
        goto cleanup;
    }
    int validation_result = backend->validate(original);
    if (validation_result == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (validation_result != 0) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }

    status = read_acl_flags(backend, original, &out->acl_flags);
    if (status == MENGXIA_ACL_OS_ERROR) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (status != MENGXIA_ACL_OK) {
        goto cleanup;
    }

    acl_entry_t entry = NULL;
    int entry_selector = ACL_FIRST_ENTRY;
    for (;;) {
        errno = 0;
        int result = backend->get_entry(original, entry_selector, &entry);
        entry_selector = ACL_NEXT_ENTRY;
        if (result == -1 && errno == EINVAL) {
            break;
        }
        if (result == -1) {
            status = status_for_errno(out);
            goto cleanup;
        }
        if (result != 0 || entry == NULL) {
            status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
            goto cleanup;
        }
        if (out->entry_count == ACL_MAX_ENTRIES) {
            status = MENGXIA_ACL_ENTRY_LIMIT_EXCEEDED;
            goto cleanup;
        }

        acl_tag_t tag = ACL_UNDEFINED_TAG;
        result = backend->get_tag_type(entry, &tag);
        if (result == -1) {
            status = status_for_errno(out);
            goto cleanup;
        }
        if (result != 0) {
            status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
            goto cleanup;
        }
        if (tag == ACL_EXTENDED_ALLOW) {
            ++out->allow_count;
        } else if (tag == ACL_EXTENDED_DENY) {
            ++out->deny_count;
        } else {
            status = MENGXIA_ACL_UNKNOWN_TAG;
            goto cleanup;
        }

        uint32_t flags = 0;
        status = read_entry_flags(backend, entry, &flags);
        if (status == MENGXIA_ACL_OS_ERROR) {
            status = status_for_errno(out);
            goto cleanup;
        }
        if (status != MENGXIA_ACL_OK) {
            goto cleanup;
        }
        entry_flags[out->entry_count] = flags;
        out->entry_flags_or |= flags;
        if ((flags & (MENGXIA_ENTRY_FILE_INHERIT |
                      MENGXIA_ENTRY_DIRECTORY_INHERIT |
                      MENGXIA_ENTRY_LIMIT_INHERIT |
                      MENGXIA_ENTRY_ONLY_INHERIT)) != 0) {
            ++out->inheritable_count;
        }
        ++out->entry_count;
    }

    errno = 0;
    ssize_t external_size = backend->size(original);
    if (external_size == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (external_size <= 0 || external_size > MENGXIA_ACL_EXTERNAL_MAX_V1) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }
    out->external_size = (uint32_t)external_size;

    duplicate = backend->duplicate(original);
    if (duplicate == NULL) {
        status = status_for_errno(out);
        goto cleanup;
    }
    validation_result = backend->validate(duplicate);
    if (validation_result == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (validation_result != 0) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }
    original_bytes = backend->allocate(MENGXIA_ACL_EXTERNAL_MAX_V1);
    duplicate_bytes = backend->allocate(MENGXIA_ACL_EXTERNAL_MAX_V1);
    if (original_bytes == NULL || duplicate_bytes == NULL) {
        errno = ENOMEM;
        status = status_for_errno(out);
        goto cleanup;
    }
    ssize_t copied =
        backend->copy_external(original_bytes, original, external_size);
    if (copied == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (copied != external_size) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }
    copied = backend->copy_external(duplicate_bytes, duplicate, external_size);
    if (copied == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (copied != external_size) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }
    if (memcmp(original_bytes, duplicate_bytes, (size_t)external_size) != 0) {
        status = MENGXIA_ACL_MALFORMED_ITERATION;
        goto cleanup;
    }

    status = restore_acl_flags(backend, duplicate, out->acl_flags);
    if (status == MENGXIA_ACL_OS_ERROR) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (status != MENGXIA_ACL_OK) {
        goto cleanup;
    }

    acl_entry_t original_entry = NULL;
    acl_entry_t duplicate_entry = NULL;
    int original_selector = ACL_FIRST_ENTRY;
    int duplicate_selector = ACL_FIRST_ENTRY;
    for (uint32_t index = 0; index < out->entry_count; ++index) {
        errno = 0;
        int original_result =
            backend->get_entry(original, original_selector, &original_entry);
        int original_errno = errno;
        errno = 0;
        int duplicate_result =
            backend->get_entry(duplicate, duplicate_selector, &duplicate_entry);
        int duplicate_errno = errno;
        original_selector = ACL_NEXT_ENTRY;
        duplicate_selector = ACL_NEXT_ENTRY;
        if (original_result == -1 || duplicate_result == -1) {
            errno = original_result == -1 ? original_errno : duplicate_errno;
            status = status_for_errno(out);
            goto cleanup;
        }
        if (original_result != 0 || duplicate_result != 0 ||
            original_entry == NULL || duplicate_entry == NULL) {
            status = MENGXIA_ACL_MALFORMED_ITERATION;
            goto cleanup;
        }
        status = restore_entry_flags(backend, duplicate_entry, entry_flags[index]);
        if (status == MENGXIA_ACL_OS_ERROR) {
            status = status_for_errno(out);
            goto cleanup;
        }
        if (status != MENGXIA_ACL_OK) {
            goto cleanup;
        }
    }
    errno = 0;
    int original_end =
        backend->get_entry(original, original_selector, &original_entry);
    int original_end_errno = errno;
    errno = 0;
    int duplicate_end =
        backend->get_entry(duplicate, duplicate_selector, &duplicate_entry);
    int duplicate_end_errno = errno;
    if (original_end != -1 || original_end_errno != EINVAL ||
        duplicate_end != -1 || duplicate_end_errno != EINVAL) {
        status = MENGXIA_ACL_MALFORMED_ITERATION;
        goto cleanup;
    }
    validation_result = backend->validate(duplicate);
    if (validation_result == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (validation_result != 0) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }
    memset(duplicate_bytes, 0, MENGXIA_ACL_EXTERNAL_MAX_V1);
    copied = backend->copy_external(duplicate_bytes, duplicate, external_size);
    if (copied == -1) {
        status = status_for_errno(out);
        goto cleanup;
    }
    if (copied != external_size) {
        status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        goto cleanup;
    }
    if (memcmp(original_bytes, duplicate_bytes, (size_t)external_size) != 0) {
        status = MENGXIA_ACL_UNKNOWN_FLAG_BITS;
        goto cleanup;
    }

cleanup:
    backend->deallocate(original_bytes);
    backend->deallocate(duplicate_bytes);
    if (duplicate != NULL) {
        int release_result = backend->release_acl(duplicate);
        if (release_result == -1 && status == MENGXIA_ACL_OK) {
            status = status_for_errno(out);
        } else if (release_result != 0 && status == MENGXIA_ACL_OK) {
            status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        }
    }
    if (original != NULL) {
        int release_result = backend->release_acl(original);
        if (release_result == -1 && status == MENGXIA_ACL_OK) {
            status = status_for_errno(out);
        } else if (release_result != 0 && status == MENGXIA_ACL_OK) {
            status = MENGXIA_ACL_UNKNOWN_SDK_RESULT;
        }
    }
    if (status != MENGXIA_ACL_OS_ERROR) {
        out->os_errno = 0;
    }
    return status;
}
