#define _DARWIN_C_SOURCE 1

#include <errno.h>
#include <fcntl.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

enum {
    MIB = 1024 * 1024,
    MAX_VM_REGIONS = 4096,
    MAX_EXTRA_FDS = 32,
};

static const rlim_t ALLOCATION_LARGE = (rlim_t)32 * MIB;
static const rlim_t ALLOCATION_SMALL = (rlim_t)8 * MIB;
static const rlim_t AS_HEADROOM = (rlim_t)16 * MIB;
static const rlim_t FIXED_AS_LIMIT = (rlim_t)2 * 1024 * MIB;

static const char *allocation_result(int success, int error) {
    return success ? "COUNTEREXAMPLE" : error == ENOMEM ? "EXPECTED" : "INCONCLUSIVE";
}

static volatile sig_atomic_t saw_sigxfsz = 0;

static void on_alarm(int signal_number) {
    (void)signal_number;
    _exit(124);
}

static void on_sigxfsz(int signal_number) {
    (void)signal_number;
    saw_sigxfsz = 1;
}

static int install_handlers(void) {
    struct sigaction alarm_action;
    struct sigaction file_action;
    memset(&alarm_action, 0, sizeof(alarm_action));
    memset(&file_action, 0, sizeof(file_action));
    alarm_action.sa_handler = on_alarm;
    file_action.sa_handler = on_sigxfsz;
    if (sigemptyset(&alarm_action.sa_mask) != 0 ||
        sigemptyset(&file_action.sa_mask) != 0 ||
        sigaction(SIGALRM, &alarm_action, NULL) != 0 ||
        sigaction(SIGXFSZ, &file_action, NULL) != 0) {
        return -1;
    }
    alarm(5);
    return 0;
}

static int disable_core(void) {
    const struct rlimit limit = {0, 0};
    return setrlimit(RLIMIT_CORE, &limit);
}

static int checked_add_rlim(rlim_t left, rlim_t right, rlim_t *result) {
    if (left == RLIM_INFINITY || right > RLIM_INFINITY - left) {
        return -1;
    }
    *result = left + right;
    return 0;
}

static int vm_inventory(uint64_t *virtual_bytes, uint32_t *regions,
                        uint64_t protection_bytes[8], int *complete) {
    mach_vm_address_t address = 0;
    uint32_t count = 0;
    uint64_t total = 0;
    memset(protection_bytes, 0, sizeof(uint64_t) * 8);
    *complete = 1;

    while (count < MAX_VM_REGIONS) {
        mach_vm_size_t size = 0;
        vm_region_basic_info_data_64_t info;
        mach_msg_type_number_t info_count = VM_REGION_BASIC_INFO_COUNT_64;
        mach_port_t object_name = MACH_PORT_NULL;
        kern_return_t result = mach_vm_region(
            mach_task_self(), &address, &size, VM_REGION_BASIC_INFO_64,
            (vm_region_info_t)&info, &info_count, &object_name);
        if (result == KERN_INVALID_ADDRESS) {
            break;
        }
        if (result != KERN_SUCCESS || size == 0) {
            return -1;
        }
        if (object_name != MACH_PORT_NULL) {
            (void)mach_port_deallocate(mach_task_self(), object_name);
        }
        if (UINT64_MAX - total < size) {
            return -1;
        }
        total += size;
        unsigned int protection = (unsigned int)info.protection & 7U;
        protection_bytes[protection] += size;
        if (UINT64_MAX - address < size) {
            return -1;
        }
        address += size;
        count += 1;
    }
    if (count == MAX_VM_REGIONS) {
        *complete = 0;
    }
    *virtual_bytes = total;
    *regions = count;
    return 0;
}

static int set_relative_as_limit(rlim_t *target) {
    uint64_t virtual_bytes = 0;
    uint32_t regions = 0;
    uint64_t protection_bytes[8];
    int complete = 0;
    struct rlimit inherited;
    if (vm_inventory(&virtual_bytes, &regions, protection_bytes, &complete) != 0 ||
        !complete || getrlimit(RLIMIT_AS, &inherited) != 0 ||
        virtual_bytes > (uint64_t)RLIM_INFINITY ||
        checked_add_rlim((rlim_t)virtual_bytes, AS_HEADROOM, target) != 0 ||
        (inherited.rlim_max != RLIM_INFINITY && *target > inherited.rlim_max)) {
        return -1;
    }
    const struct rlimit desired = {*target, *target};
    return setrlimit(RLIMIT_AS, &desired);
}

static void print_limit(const char *name, int resource) {
    struct rlimit limit;
    if (getrlimit(resource, &limit) != 0) {
        printf("%s_error=%d\n", name, errno);
        return;
    }
    printf("%s_soft=%llu\n", name, (unsigned long long)limit.rlim_cur);
    printf("%s_hard=%llu\n", name, (unsigned long long)limit.rlim_max);
}

static int baseline(void) {
    uint64_t virtual_bytes = 0;
    uint32_t regions = 0;
    uint64_t protection_bytes[8];
    int complete = 0;
    print_limit("as", RLIMIT_AS);
    print_limit("fsize", RLIMIT_FSIZE);
    print_limit("nofile", RLIMIT_NOFILE);
    if (vm_inventory(&virtual_bytes, &regions, protection_bytes, &complete) != 0) {
        puts("probe_result=INCONCLUSIVE");
        return 0;
    }
    printf("pid=%ld\nvm_regions=%u\nvirtual_bytes=%llu\nvm_complete=%d\n",
           (long)getpid(), regions, (unsigned long long)virtual_bytes, complete);
    puts(complete ? "probe_result=EXPECTED" : "probe_result=INCONCLUSIVE");
    return 0;
}

static int allocation_control(void) {
    errno = 0;
    void *allocated = malloc((size_t)ALLOCATION_LARGE);
    int malloc_errno = errno;
    errno = 0;
    void *mapped = mmap(NULL, (size_t)ALLOCATION_LARGE, PROT_READ | PROT_WRITE,
                        MAP_PRIVATE | MAP_ANON, -1, 0);
    int mmap_errno = errno;
    printf("malloc_success=%d\nmalloc_errno=%d\nmmap_success=%d\nmmap_errno=%d\n",
           allocated != NULL, malloc_errno, mapped != MAP_FAILED, mmap_errno);
    free(allocated);
    if (mapped != MAP_FAILED) {
        (void)munmap(mapped, (size_t)ALLOCATION_LARGE);
    }
    puts((allocated != NULL && mapped != MAP_FAILED) ? "probe_result=EXPECTED"
                                                      : "probe_result=INCONCLUSIVE");
    return 0;
}

static int small_control(void) {
    errno = 0;
    unsigned char *allocated = malloc((size_t)ALLOCATION_SMALL);
    int saved_errno = errno;
    if (allocated != NULL) {
        const size_t page_size = (size_t)getpagesize();
        for (size_t offset = 0; offset < (size_t)ALLOCATION_SMALL; offset += page_size) {
            allocated[offset] = (unsigned char)(offset / page_size);
        }
    }
    printf("malloc_success=%d\nerrno=%d\ntouched_bytes=%llu\n", allocated != NULL,
           saved_errno, allocated != NULL ? (unsigned long long)ALLOCATION_SMALL : 0ULL);
    free(allocated);
    puts(allocated != NULL ? "probe_result=EXPECTED" : "probe_result=INCONCLUSIVE");
    return 0;
}

static int relative_small(void) {
    rlim_t target = 0;
    if (set_relative_as_limit(&target) != 0) {
        printf("set_limit_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    errno = 0;
    unsigned char *allocated = malloc((size_t)ALLOCATION_SMALL);
    int saved_errno = errno;
    if (allocated != NULL) {
        const size_t page_size = (size_t)getpagesize();
        for (size_t offset = 0; offset < (size_t)ALLOCATION_SMALL; offset += page_size) {
            allocated[offset] = 1;
        }
    }
    printf("target=%llu\nmalloc_success=%d\nerrno=%d\ntouched_bytes=%llu\n",
           (unsigned long long)target, allocated != NULL, saved_errno,
           allocated != NULL ? (unsigned long long)ALLOCATION_SMALL : 0ULL);
    free(allocated);
    puts(allocated != NULL ? "probe_result=EXPECTED" : "probe_result=COUNTEREXAMPLE");
    return 0;
}

static int relative_large(int use_mmap) {
    rlim_t target = 0;
    if (set_relative_as_limit(&target) != 0) {
        printf("set_limit_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    errno = 0;
    void *memory = use_mmap
                       ? mmap(NULL, (size_t)ALLOCATION_LARGE, PROT_READ | PROT_WRITE,
                              MAP_PRIVATE | MAP_ANON, -1, 0)
                       : malloc((size_t)ALLOCATION_LARGE);
    int saved_errno = errno;
    int success = use_mmap ? memory != MAP_FAILED : memory != NULL;
    printf("target=%llu\nallocation_success=%d\nerrno=%d\n",
           (unsigned long long)target, success, saved_errno);
    if (success) {
        if (use_mmap) {
            (void)munmap(memory, (size_t)ALLOCATION_LARGE);
        } else {
            free(memory);
        }
    }
    printf("probe_result=%s\n", allocation_result(success, saved_errno));
    return 0;
}

static int as_exec_after(const char *encoded_target) {
    char *end = NULL;
    errno = 0;
    unsigned long long expected = strtoull(encoded_target, &end, 10);
    if (errno != 0 || end == encoded_target || *end != '\0') {
        puts("probe_result=INCONCLUSIVE");
        return 0;
    }
    struct rlimit inherited;
    if (getrlimit(RLIMIT_AS, &inherited) != 0) {
        printf("get_limit_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    rlim_t raised_max;
    if (checked_add_rlim(inherited.rlim_max, 1, &raised_max) != 0) {
        puts("probe_result=INCONCLUSIVE");
        return 0;
    }
    const struct rlimit raised = {inherited.rlim_cur, raised_max};
    errno = 0;
    int raise_result = setrlimit(RLIMIT_AS, &raised);
    int raise_errno = errno;
    errno = 0;
    void *allocated = malloc((size_t)ALLOCATION_LARGE);
    int allocation_errno = errno;
    printf("expected_target=%llu\ninherited_soft=%llu\ninherited_hard=%llu\n"
           "raise_result=%d\nraise_errno=%d\nallocation_success=%d\nallocation_errno=%d\n",
           expected, (unsigned long long)inherited.rlim_cur,
           (unsigned long long)inherited.rlim_max, raise_result, raise_errno,
           allocated != NULL, allocation_errno);
    int expected_result = inherited.rlim_cur == (rlim_t)expected &&
                          inherited.rlim_max == (rlim_t)expected &&
                          raise_result == -1 && raise_errno == EPERM &&
                          allocated == NULL && allocation_errno == ENOMEM;
    int unexpected_error = (raise_result == -1 && raise_errno != EPERM) ||
                           (allocated == NULL && allocation_errno != ENOMEM);
    free(allocated);
    puts(unexpected_error ? "probe_result=INCONCLUSIVE" :
         expected_result ? "probe_result=EXPECTED" : "probe_result=COUNTEREXAMPLE");
    return 0;
}

static int as_exec_before(const char *self_path) {
    rlim_t target = 0;
    if (set_relative_as_limit(&target) != 0) {
        printf("set_limit_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    char target_text[32];
    int written = snprintf(target_text, sizeof(target_text), "%llu",
                           (unsigned long long)target);
    if (written <= 0 || (size_t)written >= sizeof(target_text)) {
        puts("probe_result=INCONCLUSIVE");
        return 0;
    }
    fflush(NULL);
    char *const arguments[] = {(char *)self_path, (char *)"R0A_AS_EXEC_AFTER",
                               target_text, NULL};
    execv(self_path, arguments);
    printf("exec_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
    return 0;
}

static int fixed_as(void) {
    const struct rlimit desired = {FIXED_AS_LIMIT, FIXED_AS_LIMIT};
    errno = 0;
    int result = setrlimit(RLIMIT_AS, &desired);
    printf("target=%llu\nset_result=%d\nerrno=%d\n",
           (unsigned long long)FIXED_AS_LIMIT, result, errno);
    puts("probe_result=EXPECTED");
    return 0;
}

static int regions(void) {
    uint64_t virtual_bytes = 0;
    uint32_t count = 0;
    uint64_t protection_bytes[8];
    int complete = 0;
    if (vm_inventory(&virtual_bytes, &count, protection_bytes, &complete) != 0) {
        printf("inventory_error=1\nprobe_result=INCONCLUSIVE\n");
        return 0;
    }
    printf("regions=%u\nvirtual_bytes=%llu\ncomplete=%d\n", count,
           (unsigned long long)virtual_bytes, complete);
    for (unsigned int protection = 0; protection < 8; protection++) {
        printf("protection_%u_bytes=%llu\n", protection,
               (unsigned long long)protection_bytes[protection]);
    }
    puts(complete ? "probe_result=EXPECTED" : "probe_result=INCONCLUSIVE");
    return 0;
}

static ssize_t bounded_write_file(const char *name, size_t requested, int *saved_errno) {
    int descriptor = open(name, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600);
    if (descriptor < 0) {
        *saved_errno = errno;
        return -1;
    }
    unsigned char buffer[32 * 1024];
    memset(buffer, 0x5a, sizeof(buffer));
    size_t total = 0;
    while (total < requested) {
        size_t remaining = requested - total;
        size_t chunk = remaining < sizeof(buffer) ? remaining : sizeof(buffer);
        errno = 0;
        ssize_t result = write(descriptor, buffer, chunk);
        if (result <= 0) {
            *saved_errno = errno;
            break;
        }
        total += (size_t)result;
    }
    (void)close(descriptor);
    return (ssize_t)total;
}

static int file_size_limit(void) {
    struct sigaction action;
    memset(&action, 0, sizeof(action));
    action.sa_handler = on_sigxfsz;
    if (sigemptyset(&action.sa_mask) != 0 || sigaction(SIGXFSZ, &action, NULL) != 0) {
        printf("handler_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    const struct rlimit desired = {(rlim_t)64 * 1024, (rlim_t)64 * 1024};
    if (setrlimit(RLIMIT_FSIZE, &desired) != 0) {
        printf("set_limit_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    int first_errno = 0;
    int second_errno = 0;
    ssize_t first = bounded_write_file("r0a-fsize-one.bin", 96U * 1024U, &first_errno);
    ssize_t second = bounded_write_file("r0a-fsize-two.bin", 96U * 1024U, &second_errno);
    unsigned long long aggregate =
        (first > 0 ? (unsigned long long)first : 0ULL) +
        (second > 0 ? (unsigned long long)second : 0ULL);
    printf("first_bytes=%lld\nfirst_errno=%d\nsecond_bytes=%lld\nsecond_errno=%d\n"
           "aggregate_bytes=%llu\nsaw_sigxfsz=%d\n",
           (long long)first, first_errno, (long long)second, second_errno, aggregate,
           (int)saw_sigxfsz);
    int expected = first >= 0 && second >= 0 && first <= 64 * 1024 &&
                   second <= 64 * 1024 && aggregate > 64 * 1024;
    puts(expected ? "probe_result=EXPECTED" : "probe_result=COUNTEREXAMPLE");
    return 0;
}

static int descriptor_limit(void) {
    struct rlimit inherited;
    if (getrlimit(RLIMIT_NOFILE, &inherited) != 0 || inherited.rlim_max < 32) {
        printf("precondition_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    int initially_open = 0;
    for (int descriptor = 0; descriptor < 256; descriptor++) {
        if (fcntl(descriptor, F_GETFD) != -1 || errno != EBADF) {
            initially_open += 1;
        }
    }
    if (initially_open > 16) {
        printf("initially_open=%d\nprobe_result=INCONCLUSIVE\n", initially_open);
        return 0;
    }
    const struct rlimit desired = {32, 32};
    if (setrlimit(RLIMIT_NOFILE, &desired) != 0) {
        printf("set_limit_error=%d\nprobe_result=INCONCLUSIVE\n", errno);
        return 0;
    }
    int descriptors[MAX_EXTRA_FDS];
    int successes = 0;
    int final_errno = 0;
    for (int index = 0; index < MAX_EXTRA_FDS; index++) {
        errno = 0;
        int duplicated = dup(STDIN_FILENO);
        if (duplicated < 0) {
            final_errno = errno;
            break;
        }
        descriptors[successes++] = duplicated;
    }
    for (int index = 0; index < successes; index++) {
        (void)close(descriptors[index]);
    }
    printf("initially_open=%d\nsuccessful_dups=%d\nfinal_errno=%d\n", initially_open,
           successes, final_errno);
    puts(successes > 0 && final_errno == EMFILE ? "probe_result=EXPECTED"
                                                : "probe_result=INCONCLUSIVE");
    return 0;
}

static int deadline_case(void) {
    puts("deadline_probe_started=1");
    fflush(stdout);
    sleep(3);
    puts("probe_result=COUNTEREXAMPLE");
    return 0;
}

static int is_case(const char *actual, const char *expected) {
    return strcmp(actual, expected) == 0;
}

int main(int argc, char **argv) {
#ifdef R0_SELF_TEST
    if (argc == 2 && is_case(argv[1], "--self-test")) {
        if (strcmp(allocation_result(0, EINVAL), "INCONCLUSIVE") ||
            strcmp(allocation_result(0, ENOMEM), "EXPECTED") ||
            strcmp(allocation_result(1, 0), "COUNTEREXAMPLE")) return 1;
        puts("R0A_PROBE_SELF_TEST_OK assertions=3 SYNTHETIC");
        return 0;
    }
#endif
    if (disable_core() != 0 || install_handlers() != 0) {
        fprintf(stderr, "probe setup failed\n");
        return 70;
    }
    if (argc == 3 && is_case(argv[1], "R0A_AS_EXEC_AFTER")) {
        return as_exec_after(argv[2]);
    }
    if (argc != 2) {
        fprintf(stderr, "closed case id required\n");
        return 64;
    }
    if (is_case(argv[1], "R0A_BASELINE")) return baseline();
    if (is_case(argv[1], "R0A_ALLOC_CONTROL")) return allocation_control();
    if (is_case(argv[1], "R0A_SMALL_CONTROL")) return small_control();
    if (is_case(argv[1], "R0A_AS_SMALL")) return relative_small();
    if (is_case(argv[1], "R0A_AS_MALLOC")) return relative_large(0);
    if (is_case(argv[1], "R0A_AS_MMAP")) return relative_large(1);
    if (is_case(argv[1], "R0A_AS_EXEC")) return as_exec_before(argv[0]);
    if (is_case(argv[1], "R0A_AS_FIXED")) return fixed_as();
    if (is_case(argv[1], "R0A_VM_REGIONS")) return regions();
    if (is_case(argv[1], "R0A_FSIZE")) return file_size_limit();
    if (is_case(argv[1], "R0A_NOFILE")) return descriptor_limit();
    if (is_case(argv[1], "R0A_DEADLINE")) return deadline_case();
    fprintf(stderr, "unknown closed case id\n");
    return 64;
}
