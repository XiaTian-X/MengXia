/* Bounded, self-only research. No product enforcement claims. */
#define main r0a_controller_entry_not_called
#include "controller.c"
#undef main
#include <mach/mach.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sandbox.h>

/* ABI from Apple XNU f6217f8 bsd/sys/kern_memorystatus.h, absent from public
 * SDK headers. Symbol is exported by the observed SDK. Research only. */
extern int memorystatus_control(uint32_t, int32_t, uint32_t, void *, size_t);
struct memory_limits {
    int32_t active;
    uint32_t active_attributes;
    int32_t inactive;
    uint32_t inactive_attributes;
};
_Static_assert(sizeof(struct memory_limits) == 16, "memorystatus ABI size");
static const char *const CASES_B2[] = {"MEMLIMIT", "DATA"};
static const size_t SPAN_B2 = 8U * 1024U * 1024U;

static int b2_stats(uint64_t *vas, uint64_t *footprint) {
    mach_task_basic_info_data_t basic;
    task_vm_info_data_t vm;
    mach_msg_type_number_t n = MACH_TASK_BASIC_INFO_COUNT;
    if (task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&basic, &n)) return -1;
    n = TASK_VM_INFO_COUNT;
    if (task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vm, &n) ||
        n < TASK_VM_INFO_REV1_COUNT) return -1;
    *vas = basic.virtual_size;
    *footprint = vm.phys_footprint;
    return 0;
}

static int b2_memlimit(void) {
    uint64_t vas, footprint;
    if (b2_stats(&vas, &footprint) || footprint >= 32U * 1024U * 1024U) return 2;
    printf("footprint_before=%llu\n", (unsigned long long)footprint);
    struct memory_limits limits = {128, 1, 128, 1};
    errno = 0;
    int rc = memorystatus_control(7, getpid(), 0, &limits, sizeof(limits));
    int error = rc < 0 ? errno : 0;
    printf("memlimit_set_rc=%d\nmemlimit_set_errno=%d\nlimit_mb=128\n", rc, error);
    /* No follow-up allocation: success would need independent qualification. */
    puts("case_capture_complete=1");
    return 0;
}

static int b2_data(void) {
    struct rlimit small = {64U * 1024U * 1024U, 64U * 1024U * 1024U};
    errno = 0;
    int rc = setrlimit(RLIMIT_DATA, &small);
    int error = rc < 0 ? errno : 0;
    printf("data_small_rc=%d\ndata_small_errno=%d\n", rc, error);
    if (rc == 0) { puts("case_capture_complete=1"); return 0; }
    if (error != EINVAL) return 2;
    volatile unsigned char *memory = mmap(NULL, SPAN_B2, PROT_READ | PROT_WRITE,
                                         MAP_PRIVATE | MAP_ANON, -1, 0);
    if (memory == MAP_FAILED) return 2;
    char *sandbox_error = NULL;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
    rc = sandbox_init("(version 1)(deny default)", 0, &sandbox_error);
    if (sandbox_error) sandbox_free_error(sandbox_error);
#pragma clang diagnostic pop
    printf("sandbox_rc=%d\n", rc);
    if (rc) return 2;
    uint64_t vb, fb, va, fa;
    struct rlimit inherited, observed;
    if (b2_stats(&vb, &fb) || getrlimit(RLIMIT_DATA, &inherited) ||
        vb > (uint64_t)RLIM_INFINITY - 1024U * 1024U) return 2;
    rlim_t target = (rlim_t)vb + 1024U * 1024U;
    if (target > inherited.rlim_max) return 2;
    struct rlimit desired = {target, target};
    errno = 0;
    rc = setrlimit(RLIMIT_DATA, &desired);
    error = rc < 0 ? errno : 0;
    printf("data_relative_rc=%d\ndata_relative_errno=%d\ndata_target=%llu\n",
           rc, error, (unsigned long long)target);
    if (rc || getrlimit(RLIMIT_DATA, &observed) ||
        observed.rlim_cur != target || observed.rlim_max != target) return 2;
    puts("data_readback_matches=1");
    errno = 0;
    void *extra = mmap(NULL, 4U * 1024U * 1024U, PROT_READ | PROT_WRITE,
                       MAP_PRIVATE | MAP_ANON, -1, 0);
    error = extra == MAP_FAILED ? errno : 0;
    if (extra != MAP_FAILED && munmap(extra, 4U * 1024U * 1024U)) return 2;
    printf("new_mapping_errno=%d\n", error);
    uint32_t state = 0x9e3779b9U;
    for (size_t p = 0; p < SPAN_B2; p++) {
        state ^= state << 13; state ^= state >> 17; state ^= state << 5;
        memory[p] = (unsigned char)state;
    }
    struct timespec settle = {0, 20000000};
    if (nanosleep(&settle, NULL) || b2_stats(&va, &fa)) return 2;
    printf("vas_before=%llu\nvas_after=%llu\nfootprint_before=%llu\n"
           "footprint_after=%llu\ntouched_span=%zu\n",
           (unsigned long long)vb, (unsigned long long)va,
           (unsigned long long)fb, (unsigned long long)fa, SPAN_B2);
    if (munmap((void *)memory, SPAN_B2)) return 2;
    puts("case_capture_complete=1");
    return 0;
}

static int b2_private_directory(void) {
    struct stat st;
    return lstat(".", &st) == 0 && S_ISDIR(st.st_mode) &&
           st.st_uid == geteuid() && (st.st_mode & 077) == 0;
}

static int b2_parent(const char *self) {
    char root[1024];
    if (!getcwd(root, sizeof(root)) || !b2_private_directory() || self[0] != '/') return 2;
    struct sigaction action = {0}; action.sa_handler = on_cancel;
    if (sigaction(SIGTERM,&action,NULL) || sigaction(SIGINT,&action,NULL) ||
        sigaction(SIGHUP,&action,NULL)) return 2;
    double start = monotonic_seconds();
    if (start < 0) return 2;
    for (size_t c = 0; c < 2; c++) for (int attempt = 1; attempt <= 3; attempt++) {
        double now = monotonic_seconds();
        if (cancellation_signal || now < 0 || now >= start + 120) return 2;
        char outpath[4096], errpath[4096];
        int out = open_output(root,CASES_B2[c],attempt,"stdout",outpath);
        int err = open_output(root,CASES_B2[c],attempt,"stderr",errpath);
        int op[2], ep[2];
        if (out < 0 || err < 0 || pipe(op) || pipe(ep) ||
            make_nonblocking(op[0]) || make_nonblocking(ep[0])) return 2;
        pid_t child;
        if (spawn_probe(&child,self,CASES_B2[c],root,op[1],ep[1])) return 2;
        close(op[1]); close(ep[1]);
        struct stream_state streams[2]={{op[0],out,0,1,0},{ep[0],err,0,1,0}};
        int status=0, reaped=0;
        now=monotonic_seconds();
        int io_error=now < 0 ? -1 : wait_with_deadline(child,&status,&reaped,
                                                      fmin(now+10,start+120),streams,0);
        int completed=reaped;
        if (!reaped && terminate_and_reap(child,&status,&reaped,streams)) {
            fprintf(stderr,"R0B002_CLEANUP_UNCONFIRMED\n");
            while (waitpid(child,&status,0)<0 && errno==EINTR) {}
            return 2;
        }
        for (int s=0;s<2;s++) {
            while (streams[s].open && !streams[s].overflow) {
                size_t before=streams[s].bytes;
                if (drain_stream(&streams[s])) { io_error=-1; break; }
                if (streams[s].open && streams[s].bytes==before) break;
            }
            if (streams[s].open) close(streams[s].source);
            close(streams[s].destination);
        }
        printf("case=%s attempt=%d reaped=%d completed=%d exit=%d signal=%d stdout=%zu stderr=%zu\n",
               CASES_B2[c],attempt,reaped,completed,WIFEXITED(status)?WEXITSTATUS(status):-1,
               WIFSIGNALED(status)?WTERMSIG(status):0,streams[0].bytes,streams[1].bytes);
        fflush(stdout);
        if (!completed || io_error || cancellation_signal || streams[0].overflow ||
            streams[1].overflow || !WIFEXITED(status) || WEXITSTATUS(status)) return 2;
    }
    puts("R0B002_CAPTURE_COMPLETE_NOT_QUALIFICATION");
    return 0;
}

int main(int argc, char **argv) {
    if (getuid() == 0 || geteuid() == 0 || !b2_private_directory()) return 2;
    if (argc == 1) return b2_parent(argv[0]);
    if (argc != 2) return 64;
    struct rlimit core = {0,0};
    if (setrlimit(RLIMIT_CORE,&core)) return 2;
    alarm(5);
    setvbuf(stdout,NULL,_IONBF,0);
    printf("uid=%u\neuid=%u\n", (unsigned)getuid(), (unsigned)geteuid());
    if (!strcmp(argv[1],"MEMLIMIT")) return b2_memlimit();
    if (!strcmp(argv[1],"DATA")) return b2_data();
    return 64;
}
