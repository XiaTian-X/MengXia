/* Research only: reuse the reviewed supervisor; do not change the R0-A binary. */
#define main r0a_controller_entry_not_called
#include "controller.c"
#undef main
#include <mach/mach.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sandbox.h>
#include <ctype.h>

static const char *const B_CASES[] = {"CONTROL", "SANDBOX", "RESERVED_MEMORY"};
static const size_t B_MEMORY = 8U * 1024U * 1024U;

static int private_directory(const char *path) {
    struct stat st;
    return lstat(path, &st) == 0 && S_ISDIR(st.st_mode) &&
           st.st_uid == geteuid() && (st.st_mode & 077) == 0;
}

static int install_b_sandbox(void) {
    char cwd[1024], profile[4096];
    if (!getcwd(cwd, sizeof(cwd))) return -1;
    /* Closed mktemp names only; never interpolate arbitrary SBPL strings. */
    for (const unsigned char *p = (const unsigned char *)cwd; *p; p++)
        if (!isalnum(*p) && *p != '/' && *p != '-' && *p != '_' && *p != '.') return -1;
    int n = snprintf(profile, sizeof(profile),
        "(version 1)(deny default)"
        "(allow file-read-metadata (literal \"%s\") (literal \"%s/slot-a\") (literal \"%s/slot-b\"))"
        "(allow file-read-data file-write-data (literal \"%s/slot-a\") (literal \"%s/slot-b\"))",
        cwd, cwd, cwd, cwd, cwd);
    if (n < 0 || (size_t)n >= sizeof(profile)) return -1;
    char *error = NULL;
    /* Deliberately unsupported/deprecated research API, never a product promise. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    int result = sandbox_init(profile, 0, &error);
    if (error) sandbox_free_error(error);
#pragma clang diagnostic pop
    printf("sandbox_rc=%d\n", result);
    return result;
}

static long write_slot(const char *name, int *error) {
    int fd = open(name, O_WRONLY | O_NOFOLLOW);
    if (fd < 0) { *error = errno; return -1; }
    unsigned char bytes[32768];
    memset(bytes, 0x5a, sizeof(bytes));
    long total = 0;
    *error = 0;
    for (int i = 0; i < 3; i++) {
        ssize_t n = write(fd, bytes, sizeof(bytes));
        if (n < 0) { *error = errno; break; }
        total += n;
        if (n != sizeof(bytes)) break;
    }
    if (close(fd) != 0) *error = errno;
    return total;
}

static int filesystem_probe(int sandboxed) {
    struct rlimit fsize = {65536, 65536};
    if (setrlimit(RLIMIT_FSIZE, &fsize) || signal(SIGXFSZ, SIG_IGN) == SIG_ERR) return 2;
    if (sandboxed && install_b_sandbox()) return 2;
    if (!sandboxed) puts("sandbox_rc=NOT_APPLIED_CONTROL");
    int e1, e2;
    long n1 = write_slot("slot-a", &e1), n2 = write_slot("slot-b", &e2);
    printf("slot_a_bytes=%ld\nslot_a_errno=%d\nslot_b_bytes=%ld\nslot_b_errno=%d\n", n1,e1,n2,e2);
    errno = 0;
    int fd = open("canary", O_RDONLY | O_NOFOLLOW);
    int read_error = fd < 0 ? errno : 0;
    if (fd >= 0) { unsigned char byte; if (read(fd, &byte, 1) != 1) read_error = EIO; close(fd); }
    printf("canary_errno=%d\n", read_error);
    errno = 0;
    fd = open("third-slot", O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600);
    int create_error = fd < 0 ? errno : 0;
    if (fd >= 0) close(fd); /* Zero bytes even if the policy unexpectedly allows it. */
    printf("create_errno=%d\n", create_error);
    errno = 0;
    int renamed = rename("slot-a", "moved-slot");
    int rename_error = renamed < 0 ? errno : 0;
    if (renamed == 0 && rename("moved-slot", "slot-a") != 0) return 2;
    printf("rename_errno=%d\n", rename_error);
    fflush(stdout);
    errno = 0;
    pid_t pid = fork();
    int fork_error = pid < 0 ? errno : 0;
    if (pid == 0) _exit(0);
    if (pid > 0) {
        int status;
        if (waitpid(pid, &status, 0) != pid || !WIFEXITED(status) || WEXITSTATUS(status)) return 2;
    }
    printf("fork_errno=%d\n", fork_error);
    return 0; /* Observation capture, not enforcement PASS. */
}

static int own_stats(uint64_t *vas, uint64_t *resident, uint64_t *footprint) {
    mach_task_basic_info_data_t basic;
    task_vm_info_data_t vm;
    mach_msg_type_number_t n = MACH_TASK_BASIC_INFO_COUNT;
    kern_return_t kr = task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&basic, &n);
    if (kr != KERN_SUCCESS) { printf("basic_info_error=%d\n", kr); return -1; }
    n = TASK_VM_INFO_COUNT;
    kr = task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vm, &n);
    if (kr != KERN_SUCCESS || n < TASK_VM_INFO_REV1_COUNT) { printf("vm_info_error=%d\n", kr); return -1; }
    *vas = basic.virtual_size; *resident = basic.resident_size; *footprint = vm.phys_footprint;
    return 0;
}

static int memory_probe(void) {
    /* Own allocation only. Never touch or reprotect dyld/runtime-reserved regions. */
    volatile unsigned char *memory = mmap(NULL, B_MEMORY, PROT_READ | PROT_WRITE,
                                         MAP_PRIVATE | MAP_ANON, -1, 0);
    if (memory == MAP_FAILED) return 2;
    if (install_b_sandbox()) { munmap((void *)memory, B_MEMORY); return 2; }
    uint64_t vb, rb, fb, va, ra, fa;
    if (own_stats(&vb, &rb, &fb)) return 2;
    struct rlimit inherited;
    if (getrlimit(RLIMIT_AS, &inherited) || vb > (uint64_t)RLIM_INFINITY - 1024U*1024U) return 2;
    rlim_t target = (rlim_t)vb + 1024U*1024U;
    if (target > inherited.rlim_max) return 2;
    struct rlimit desired = {target, target};
    errno = 0;
    int limited = setrlimit(RLIMIT_AS, &desired);
    printf("set_as_rc=%d\nset_as_errno=%d\nas_target=%llu\n", limited, errno, (unsigned long long)target);
    if (limited) return 2;
    errno = 0;
    void *extra = mmap(NULL, 4U*1024U*1024U, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    int extra_error = extra == MAP_FAILED ? errno : 0;
    if (extra != MAP_FAILED) munmap(extra, 4U*1024U*1024U);
    printf("new_mapping_errno=%d\n", extra_error);
    /* Fill the owned mapping with bounded nonzero, non-repeating data; record
       this separately from the earlier sparse-page-touch observation. */
    uint32_t state = 0x9e3779b9U;
    for (size_t p = 0; p < B_MEMORY; p++) {
        state ^= state << 13; state ^= state >> 17; state ^= state << 5;
        memory[p] = (unsigned char)state;
    }
    struct timespec settle = {0, 20000000};
    if (nanosleep(&settle, NULL)) return 2;
    if (own_stats(&va, &ra, &fa)) return 2;
    printf("vas_before=%llu\nvas_after=%llu\nresident_before=%llu\nresident_after=%llu\n"
           "footprint_before=%llu\nfootprint_after=%llu\ntouched_span=%zu\n",
           (unsigned long long)vb,(unsigned long long)va,(unsigned long long)rb,(unsigned long long)ra,
           (unsigned long long)fb,(unsigned long long)fa,B_MEMORY);
    return munmap((void *)memory, B_MEMORY) == 0 ? 0 : 2;
}

static int b_child(const char *mode) {
    struct rlimit core = {0,0};
    if (!private_directory(".") || setrlimit(RLIMIT_CORE, &core)) return 2;
    alarm(5);
    setvbuf(stdout, NULL, _IONBF, 0);
    if (!strcmp(mode,"CONTROL")) return filesystem_probe(0);
    if (!strcmp(mode,"SANDBOX")) return filesystem_probe(1);
    if (!strcmp(mode,"RESERVED_MEMORY")) return memory_probe();
    return 64;
}

static int b_parent(const char *self) {
    char root[1024];
    if (!getcwd(root, sizeof(root)) || !private_directory(root) || self[0] != '/') return 2;
    struct sigaction action = {0}; action.sa_handler = on_cancel;
    if (sigaction(SIGTERM,&action,NULL) || sigaction(SIGINT,&action,NULL) || sigaction(SIGHUP,&action,NULL)) return 2;
    double start = monotonic_seconds();
    if (start < 0) return 2;
    for (size_t c = 0; c < 3; c++) for (int attempt = 1; attempt <= 3; attempt++) {
        if (cancellation_signal || monotonic_seconds() >= start + 120) return 2;
        char work[1200], path[1600];
        int n = snprintf(work,sizeof(work),"%s/work-%s-%d",root,B_CASES[c],attempt);
        if (n < 0 || (size_t)n >= sizeof(work) || mkdir(work,0700)) return 2;
        const char *names[] = {"slot-a","slot-b","canary"};
        for (int f = 0; f < 3; f++) {
            snprintf(path,sizeof(path),"%s/%s",work,names[f]);
            int fd = open(path,O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW,0600);
            if (fd < 0) return 2;
            if (f == 2 && write(fd,"X",1) != 1) { close(fd); return 2; }
            if (close(fd)) return 2;
        }
        char outpath[4096], errpath[4096];
        int out = open_output(root,B_CASES[c],attempt,"stdout",outpath);
        int err = open_output(root,B_CASES[c],attempt,"stderr",errpath);
        int op[2], ep[2];
        if (out < 0 || err < 0 || pipe(op) || pipe(ep) || make_nonblocking(op[0]) || make_nonblocking(ep[0])) return 2;
        pid_t child;
        if (spawn_probe(&child,self,B_CASES[c],work,op[1],ep[1])) return 2;
        close(op[1]); close(ep[1]);
        struct stream_state streams[2]={{op[0],out,0,1,0},{ep[0],err,0,1,0}};
        int status=0, reaped=0;
        double now=monotonic_seconds(), deadline=fmin(now+10,start+120);
        int io_error = now < 0 ? -1 : wait_with_deadline(child,&status,&reaped,deadline,streams,0);
        int completed = reaped;
        if (!reaped && terminate_and_reap(child,&status,&reaped,streams)) {
            fprintf(stderr,"R0B001_CLEANUP_UNCONFIRMED\n");
            while (waitpid(child,&status,0)<0 && errno==EINTR) {}
            return 2;
        }
        for(int s=0;s<2;s++) {
            while (streams[s].open && !streams[s].overflow) {
                size_t before=streams[s].bytes;
                if (drain_stream(&streams[s])) { io_error=-1; break; }
                if(streams[s].open && streams[s].bytes==before) break;
            }
            if(streams[s].open) close(streams[s].source);
            close(streams[s].destination);
        }
        printf("case=%s attempt=%d reaped=%d completed=%d exit=%d signal=%d stdout=%zu stderr=%zu\n",
               B_CASES[c],attempt,reaped,completed,WIFEXITED(status)?WEXITSTATUS(status):-1,
               WIFSIGNALED(status)?WTERMSIG(status):0,streams[0].bytes,streams[1].bytes);
        for (int f=0;f<2;f++) {
            snprintf(path,sizeof(path),"%s/%s",work,names[f]);
            int fd=open(path,O_RDONLY|O_NOFOLLOW|O_NONBLOCK); struct stat st;
            if(fd<0 || fstat(fd,&st) || !S_ISREG(st.st_mode)) return 2;
            close(fd);
            printf("case=%s attempt=%d parent_%s_size=%lld\n",B_CASES[c],attempt,names[f],(long long)st.st_size);
            if(st.st_size != (c==2 ? 0 : 65536)) return 2;
        }
        fflush(stdout);
        if(!completed || io_error || cancellation_signal || streams[0].overflow || streams[1].overflow ||
           !WIFEXITED(status) || WEXITSTATUS(status)) return 2;
    }
    puts("R0B001_CAPTURE_COMPLETE_NOT_QUALIFICATION");
    return 0;
}

int main(int argc,char **argv) {
    if(argc==1) return b_parent(argv[0]);
    if(argc==2) for(size_t i=0;i<3;i++) if(!strcmp(argv[1],B_CASES[i])) return b_child(argv[1]);
    return 64;
}
