#include "mengxia_acl_shim.h"

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/acl.h>

#define FAKE_ENTRY_MAX 129u

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

extern int32_t mengxia_acl_inspect_fd_core_v1(
    int32_t fd, struct mengxia_acl_summary_v1 *out,
    const struct mengxia_acl_backend_v1 *backend)
    __attribute__((visibility("hidden")));

struct fake_flag_object {
    uint32_t kind;
    uint32_t flags;
    uint32_t hidden_flags;
};

struct fake_entry {
    struct fake_flag_object header;
    acl_tag_t tag;
};

struct fake_acl {
    struct fake_flag_object header;
    uint32_t count;
    uint32_t cursor;
    struct fake_entry entries[FAKE_ENTRY_MAX];
};

static struct fake_acl original_acl;
static struct fake_acl duplicate_acl;
static int get_fd_error;
static ssize_t size_override;
static int duplicate_external_mismatch;
static uint32_t release_count;
static uint32_t allocation_count;
static uint32_t deallocation_count;

static acl_t fake_get_fd(int fd, acl_type_t type) {
    if (fd < 0 || type != ACL_TYPE_EXTENDED) {
        errno = EINVAL;
        return NULL;
    }
    if (get_fd_error != 0) {
        errno = get_fd_error;
        return NULL;
    }
    return (acl_t)&original_acl;
}

static int fake_get_entry(acl_t value, int selector, acl_entry_t *out) {
    struct fake_acl *acl = (struct fake_acl *)value;
    if (acl == NULL || out == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (selector == ACL_FIRST_ENTRY) {
        acl->cursor = 0;
    } else if (selector != ACL_NEXT_ENTRY) {
        errno = EINVAL;
        return -1;
    }
    if (acl->cursor >= acl->count) {
        errno = EINVAL;
        return -1;
    }
    *out = (acl_entry_t)&acl->entries[acl->cursor];
    ++acl->cursor;
    return 0;
}

static int fake_get_tag_type(acl_entry_t value, acl_tag_t *out) {
    if (value == NULL || out == NULL) {
        errno = EINVAL;
        return -1;
    }
    *out = ((struct fake_entry *)value)->tag;
    return 0;
}

static int fake_get_flagset(void *value, acl_flagset_t *out) {
    if (value == NULL || out == NULL) {
        errno = EINVAL;
        return -1;
    }
    *out = (acl_flagset_t)value;
    return 0;
}

static int fake_get_flag(acl_flagset_t value, acl_flag_t flag) {
    if (value == NULL) {
        errno = EINVAL;
        return -1;
    }
    const struct fake_flag_object *object =
        (const struct fake_flag_object *)value;
    return (object->flags & (uint32_t)flag) != 0 ? 1 : 0;
}

static int fake_clear_flags(acl_flagset_t value) {
    if (value == NULL) {
        errno = EINVAL;
        return -1;
    }
    struct fake_flag_object *object = (struct fake_flag_object *)value;
    object->flags = 0;
    object->hidden_flags = 0;
    return 0;
}

static int fake_add_flag(acl_flagset_t value, acl_flag_t flag) {
    if (value == NULL) {
        errno = EINVAL;
        return -1;
    }
    ((struct fake_flag_object *)value)->flags |= (uint32_t)flag;
    return 0;
}

static acl_t fake_duplicate(acl_t value) {
    if (value == NULL) {
        errno = EINVAL;
        return NULL;
    }
    duplicate_acl = *(struct fake_acl *)value;
    duplicate_acl.cursor = 0;
    if (duplicate_external_mismatch) {
        duplicate_acl.header.hidden_flags ^= 0x40000000u;
    }
    return (acl_t)&duplicate_acl;
}

static int fake_validate(acl_t value) {
    if (value == NULL) {
        errno = EINVAL;
        return -1;
    }
    ((struct fake_acl *)value)->cursor = 0;
    return 0;
}

static ssize_t fake_size(acl_t value) {
    if (value == NULL) {
        errno = EINVAL;
        return -1;
    }
    if (size_override != 0) {
        return size_override;
    }
    const struct fake_acl *acl = (const struct fake_acl *)value;
    return (ssize_t)(16u + acl->count * 12u);
}

static ssize_t fake_copy_external(void *buffer, acl_t value, ssize_t size) {
    if (buffer == NULL || value == NULL || size != fake_size(value)) {
        errno = EINVAL;
        return -1;
    }
    const struct fake_acl *acl = (const struct fake_acl *)value;
    unsigned char *bytes = buffer;
    memset(bytes, 0, (size_t)size);
    memcpy(bytes, &acl->header.flags, sizeof(uint32_t));
    memcpy(bytes + 4, &acl->header.hidden_flags, sizeof(uint32_t));
    memcpy(bytes + 8, &acl->count, sizeof(uint32_t));
    for (uint32_t index = 0; index < acl->count; ++index) {
        size_t offset = 16u + (size_t)index * 12u;
        memcpy(bytes + offset, &acl->entries[index].tag, sizeof(uint32_t));
        memcpy(bytes + offset + 4, &acl->entries[index].header.flags,
               sizeof(uint32_t));
        memcpy(bytes + offset + 8,
               &acl->entries[index].header.hidden_flags, sizeof(uint32_t));
    }
    return size;
}

static int fake_release(void *value) {
    if (value == NULL) {
        errno = EINVAL;
        return -1;
    }
    ++release_count;
    return 0;
}

static void *fake_allocate(size_t size) {
    ++allocation_count;
    return malloc(size);
}

static void fake_deallocate(void *value) {
    if (value != NULL) {
        ++deallocation_count;
    }
    free(value);
}

static const struct mengxia_acl_backend_v1 fake_backend = {
    .get_fd = fake_get_fd,
    .get_entry = fake_get_entry,
    .get_tag_type = fake_get_tag_type,
    .get_flagset = fake_get_flagset,
    .get_flag = fake_get_flag,
    .clear_flags = fake_clear_flags,
    .add_flag = fake_add_flag,
    .duplicate = fake_duplicate,
    .validate = fake_validate,
    .size = fake_size,
    .copy_external = fake_copy_external,
    .release_acl = fake_release,
    .allocate = fake_allocate,
    .deallocate = fake_deallocate,
};

static void reset_fixture(void) {
    memset(&original_acl, 0, sizeof(original_acl));
    memset(&duplicate_acl, 0, sizeof(duplicate_acl));
    original_acl.header.kind = 1;
    for (uint32_t index = 0; index < FAKE_ENTRY_MAX; ++index) {
        original_acl.entries[index].header.kind = 2;
        original_acl.entries[index].tag = ACL_EXTENDED_DENY;
    }
    get_fd_error = 0;
    size_override = 0;
    duplicate_external_mismatch = 0;
    release_count = 0;
    allocation_count = 0;
    deallocation_count = 0;
}

static int expect(int condition, const char *message, int line) {
    if (!condition) {
        fprintf(stderr, "line %d: %s\n", line, message);
        return 0;
    }
    return 1;
}

#define EXPECT(condition, message, ignored)                                     \
    do {                                                                        \
        (void)(ignored);                                                        \
        if (!expect((condition), (message), __LINE__)) {                        \
            return 1;                                                           \
        }                                                                       \
    } while (0)

int main(void) {
    EXPECT(mengxia_acl_path_is_empty_v1(NULL, NULL) == 1,
           "path ACL probe rejects null arguments", 0);
    struct mengxia_acl_summary_v1 summary;

    reset_fixture();
    EXPECT(mengxia_acl_inspect_fd_core_v1(-1, &summary, &fake_backend) == 1,
           "negative fd must be invalid", 0);

    reset_fixture();
    get_fd_error = ENOENT;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 0,
           "ENOENT on an open fd must normalize to empty ACL", 0);
    EXPECT(summary.entry_count == 0 && summary.external_size == 0 &&
               summary.os_errno == 0,
           "empty ACL summary must be all-zero evidence", 0);

    reset_fixture();
    original_acl.count = 1;
    original_acl.header.flags = ACL_FLAG_NO_INHERIT;
    original_acl.entries[0].header.flags =
        ACL_ENTRY_INHERITED | ACL_ENTRY_FILE_INHERIT;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 0,
           "known deny ACL must validate", 0);
    EXPECT(summary.entry_count == 1 && summary.deny_count == 1 &&
               summary.allow_count == 0 && summary.acl_flags == 2 &&
               summary.entry_flags_or == 3 &&
               summary.inheritable_count == 1,
           "known ACL summary fields must remain distinct", 0);
    EXPECT(release_count == 2 && allocation_count == 2 &&
               deallocation_count == 2,
           "success must release both ACLs and buffers exactly once", 0);

    reset_fixture();
    original_acl.count = 1;
    original_acl.entries[0].tag = ACL_EXTENDED_ALLOW;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 0 &&
               summary.allow_count == 1 && summary.deny_count == 0,
           "allow tags must be reported without policy interpretation", 0);

    reset_fixture();
    original_acl.header.hidden_flags = 0x80000000u;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 7,
           "unknown object flag bits must fail reconstruction", 0);
    EXPECT(release_count == 2 && deallocation_count == 2,
           "unknown flags must still clean up", 0);

    reset_fixture();
    original_acl.count = 1;
    original_acl.entries[0].header.hidden_flags = 0x80000000u;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 7,
           "unknown entry flag bits must fail reconstruction", 0);
    EXPECT(release_count == 2 && deallocation_count == 2,
           "unknown entry flags must still clean up", 0);

    reset_fixture();
    original_acl.count = ACL_MAX_ENTRIES;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 0,
           "exactly 128 entries must complete", 0);
    EXPECT(summary.entry_count == ACL_MAX_ENTRIES &&
               summary.deny_count == ACL_MAX_ENTRIES,
           "the exact finite entry bound must be observable", 0);
    EXPECT(release_count == 2 && allocation_count == 2 &&
               deallocation_count == 2,
           "the exact-bound success path must clean up once", 0);

    reset_fixture();
    original_acl.count = FAKE_ENTRY_MAX;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 6,
           "the 129th entry must fail immediately", 0);
    EXPECT(release_count == 1 && allocation_count == 0,
           "entry-limit failure must remain bounded", 0);

    reset_fixture();
    original_acl.count = 1;
    size_override = MENGXIA_ACL_EXTERNAL_MAX_V1 + 1;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 5,
           "external ACL representation above 16384 bytes must fail", 0);
    EXPECT(release_count == 1 && allocation_count == 0,
           "oversize external representation must fail before allocation", 0);

    reset_fixture();
    original_acl.count = 1;
    duplicate_external_mismatch = 1;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 3,
           "a duplicate with different external bytes is malformed", 0);
    EXPECT(release_count == 2 && allocation_count == 2 &&
               deallocation_count == 2,
           "external mismatch must clean up every object once", 0);

    reset_fixture();
    original_acl.count = 1;
    original_acl.entries[0].tag = ACL_UNDEFINED_TAG;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 4,
           "unknown tags must fail closed", 0);

    reset_fixture();
    get_fd_error = EACCES;
    EXPECT(mengxia_acl_inspect_fd_core_v1(3, &summary, &fake_backend) == 2 &&
               summary.os_errno == EACCES,
           "OS errors must preserve only errno in private ABI", 0);

    return 0;
}
