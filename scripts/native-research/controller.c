#define _DARWIN_C_SOURCE 1

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <spawn.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum {
    STREAM_LIMIT = 32 * 1024,
    NORMAL_TIMEOUT_SECONDS = 10,
    DEADLINE_TIMEOUT_SECONDS = 1,
    TERM_GRACE_SECONDS = 1,
    KILL_GRACE_SECONDS = 1,
};

static volatile sig_atomic_t cancellation_signal = 0;

static const char *const CASES[] = {
    "R0A_BASELINE",      "R0A_ALLOC_CONTROL", "R0A_SMALL_CONTROL",
    "R0A_AS_SMALL",      "R0A_AS_MALLOC",     "R0A_AS_MMAP",
    "R0A_AS_EXEC",       "R0A_AS_FIXED",      "R0A_VM_REGIONS",
    "R0A_FSIZE",         "R0A_NOFILE",        "R0A_DEADLINE",
};

struct stream_state {
    int source;
    int destination;
    size_t bytes;
    int open;
    int overflow;
};

static void on_cancel(int signal_number) {
    cancellation_signal = signal_number;
}

static int valid_case(const char *candidate) {
    for (size_t index = 0; index < sizeof(CASES) / sizeof(CASES[0]); index++) {
        if (strcmp(candidate, CASES[index]) == 0) return 1;
    }
    return 0;
}

static int valid_attempt(const char *candidate, int *attempt) {
    if (candidate[0] < '1' || candidate[0] > '3' || candidate[1] != '\0') return 0;
    *attempt = candidate[0] - '0';
    return 1;
}

static int make_nonblocking(int descriptor) {
    int flags = fcntl(descriptor, F_GETFL);
    return flags >= 0 && fcntl(descriptor, F_SETFL, flags | O_NONBLOCK) == 0 ? 0 : -1;
}

static double monotonic_seconds(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value) != 0) return -1.0;
    return (double)value.tv_sec + (double)value.tv_nsec / 1000000000.0;
}

static int safe_write(int descriptor, const unsigned char *buffer, size_t length) {
    size_t offset = 0;
    while (offset < length) {
        ssize_t result = write(descriptor, buffer + offset, length - offset);
        if (result > 0) {
            offset += (size_t)result;
        } else if (result < 0 && errno == EINTR) {
            continue;
        } else {
            return -1;
        }
    }
    return 0;
}

static int drain_stream(struct stream_state *stream) {
    unsigned char buffer[4096];
    while (stream->open) {
        ssize_t result = read(stream->source, buffer, sizeof(buffer));
        if (result > 0) {
            size_t available = STREAM_LIMIT - stream->bytes;
            size_t retained = (size_t)result < available ? (size_t)result : available;
            if (retained > 0 && safe_write(stream->destination, buffer, retained) != 0) {
                return -1;
            }
            stream->bytes += retained;
            if ((size_t)result > retained) {
                stream->overflow = 1;
                return 0;
            }
            /* Return to the deadline/cleanup loop after each bounded read. */
            break;
        } else if (result == 0) {
            stream->open = 0;
            (void)close(stream->source);
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
            break;
        } else if (errno != EINTR) {
            return -1;
        }
    }
    return 0;
}

static int wait_nonblocking(pid_t child, int *status, int *reaped) {
    pid_t result = waitpid(child, status, WNOHANG);
    if (result == child) {
        *reaped = 1;
        return 0;
    }
    if (result == 0 || (result < 0 && errno == EINTR)) return 0;
    return -1;
}

static int wait_with_deadline(pid_t child, int *status, int *reaped, double deadline,
                              struct stream_state streams[2], int cleaning) {
    while (!*reaped) {
        double now = monotonic_seconds();
        if (now < 0) return -1;
        if (now >= deadline) break;
        if (!cleaning && (cancellation_signal != 0 || streams[0].overflow || streams[1].overflow)) break;
        struct pollfd descriptors[2];
        nfds_t count = 0;
        for (size_t index = 0; index < 2; index++) {
            if (streams[index].open) {
                descriptors[count].fd = streams[index].source;
                descriptors[count].events = POLLIN | POLLHUP;
                descriptors[count].revents = 0;
                count += 1;
            }
        }
        (void)poll(descriptors, count, 25);
        for (size_t index = 0; index < 2; index++) {
            if (streams[index].open && !streams[index].overflow &&
                drain_stream(&streams[index]) != 0) {
                if (!cleaning) return -1;
                /* Output failure must never suppress waitpid/termination. */
                streams[index].overflow = 1;
            }
        }
        if (!cleaning && (streams[0].overflow || streams[1].overflow)) break;
        if (wait_nonblocking(child, status, reaped) != 0) return -1;
    }
    return 0;
}

static int terminate_and_reap(pid_t child, int *status, int *reaped,
                              struct stream_state streams[2]) {
    if (!*reaped && kill(child, SIGTERM) != 0 && errno != ESRCH) return -1;
    double deadline = monotonic_seconds() + TERM_GRACE_SECONDS;
    (void)wait_with_deadline(child, status, reaped, deadline, streams, 1);
    if (!*reaped && kill(child, SIGKILL) != 0 && errno != ESRCH) return -1;
    deadline = monotonic_seconds() + KILL_GRACE_SECONDS;
    (void)wait_with_deadline(child, status, reaped, deadline, streams, 1);
    if (!*reaped) (void)wait_nonblocking(child, status, reaped);
    return *reaped ? 0 : -1;
}

static int open_output(const char *evidence_directory, const char *case_id, int attempt,
                       const char *suffix, char path[4096]) {
    int written = snprintf(path, 4096, "%s/case-%s-%d.%s", evidence_directory, case_id,
                           attempt, suffix);
    if (written <= 0 || written >= 4096) {
        errno = ENAMETOOLONG;
        return -1;
    }
    return open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600);
}

static int spawn_probe(pid_t *child, const char *probe, const char *case_id,
                       const char *work, int out, int err) {
    posix_spawn_file_actions_t actions;
    posix_spawnattr_t attributes;
    int result = posix_spawn_file_actions_init(&actions);
    if (result != 0) return result;
    result = posix_spawnattr_init(&attributes);
    if (result != 0) { posix_spawn_file_actions_destroy(&actions); return result; }
    sigset_t empty, defaults;
    sigemptyset(&empty);
    sigfillset(&defaults);
    /* CLOEXEC_DEFAULT closes every non-whitelisted FD, including high numbers. */
    if ((result = posix_spawnattr_setflags(&attributes, POSIX_SPAWN_CLOEXEC_DEFAULT |
            POSIX_SPAWN_SETSIGMASK | POSIX_SPAWN_SETSIGDEF)) == 0 &&
        (result = posix_spawnattr_setsigmask(&attributes, &empty)) == 0 &&
        (result = posix_spawnattr_setsigdefault(&attributes, &defaults)) == 0 &&
        (result = posix_spawn_file_actions_addopen(&actions, STDIN_FILENO, "/dev/null", O_RDONLY, 0)) == 0 &&
        (result = posix_spawn_file_actions_adddup2(&actions, out, STDOUT_FILENO)) == 0 &&
        (result = posix_spawn_file_actions_adddup2(&actions, err, STDERR_FILENO)) == 0 &&
        (result = posix_spawn_file_actions_addchdir(&actions, work)) == 0) {
        char *const arguments[] = {(char *)probe, (char *)case_id, NULL};
        char *const environment[] = {(char *)"LANG=C", (char *)"LC_ALL=C", (char *)"TZ=UTC", NULL};
        result = posix_spawn(child, probe, &actions, &attributes, arguments, environment);
    }
    posix_spawnattr_destroy(&attributes);
    posix_spawn_file_actions_destroy(&actions);
    return result;
}

#ifdef R0_SELF_TEST
static int controller_self_test(const char *self) {
    alarm(20);
    for (int mode = 0; mode < 3; mode++) {
        int status = 0, reaped = 0;
        int ready[2];
        if (pipe(ready)) return 1;
        pid_t child = fork();
        if (child < 0) return 1;
        if (child == 0) {
            alarm(3);
            if (mode == 2) signal(SIGTERM, SIG_IGN);
            close(ready[0]);
            (void)write(ready[1], "R", 1);
            close(ready[1]);
            sleep(2);
            _exit(0);
        }
        close(ready[1]);
        char byte;
        if (read(ready[0], &byte, 1) != 1) { kill(child, SIGKILL); waitpid(child, NULL, 0); return 1; }
        close(ready[0]);
        struct stream_state streams[2] = {{-1, -1, 0, 0, mode == 1}, {-1, -1, 0, 0, 0}};
        cancellation_signal = mode == 0 ? SIGTERM : 0;
        int result = terminate_and_reap(child, &status, &reaped, streams);
        if (result || !reaped) { kill(child, SIGKILL); waitpid(child, NULL, 0); return 2; }
        if (waitpid(child, NULL, WNOHANG) != -1 || errno != ECHILD) return 3;
        if (mode == 2 && (!WIFSIGNALED(status) || WTERMSIG(status) != SIGKILL)) return 3;
        cancellation_signal = 0;
    }
    int fd = open("/dev/null", O_RDWR);
    if (fd < 0 || dup2(fd, 5000) < 0) return 4;
    pid_t child;
    if (spawn_probe(&child, self, "--fd-child", "/tmp", fd, fd)) return 5;
    close(5000);
    close(fd);
    int status;
    if (waitpid(child, &status, 0) != child || !WIFEXITED(status) || WEXITSTATUS(status)) return 6;
    int output[2];
    if (pipe(output) || make_nonblocking(output[0])) return 7;
    fd = open("/dev/null", O_WRONLY);
    if (fd < 0) return 7;
    child = fork();
    if (child < 0) return 7;
    if (child == 0) {
        alarm(3); close(output[0]);
        unsigned char bytes[4096] = {0};
        for (int i=0; i<16; i++) if (safe_write(output[1], bytes, sizeof(bytes))) _exit(1);
        close(output[1]); sleep(2); _exit(0);
    }
    close(output[1]);
    struct stream_state streams[2] = {{output[0], fd, 0, 1, 0}, {-1, -1, 0, 0, 0}};
    int reaped = 0;
    int result = wait_with_deadline(child, &status, &reaped, monotonic_seconds()+2, streams, 0);
    int cleaned = terminate_and_reap(child, &status, &reaped, streams);
    close(output[0]); close(fd);
    if (result || cleaned || !reaped || !streams[0].overflow || streams[0].bytes != STREAM_LIMIT) return 8;
    puts("R0A_CONTROLLER_SELF_TEST_OK cases=5 SYNTHETIC");
    return 0;
}
#endif

int main(int argc, char **argv) {
#ifdef R0_SELF_TEST
    if (argc == 2 && strcmp(argv[1], "--self-test") == 0) return controller_self_test(argv[0]);
    if (argc == 2 && strcmp(argv[1], "--fd-child") == 0)
        return fcntl(5000, F_GETFD) == -1 && errno == EBADF ? 0 : 1;
#endif
    if (argc == 2 && strcmp(argv[1], "--clock") == 0) {
        double now = monotonic_seconds();
        if (now < 0) return 70;
        printf("%.6f\n", now);
        return 0;
    }
    if (argc != 7) {
        fprintf(stderr, "usage: controller PROBE CASE ATTEMPT EVIDENCE_DIR WORK_DIR BATCH_DEADLINE\n");
        return 64;
    }
    char *end = NULL;
    double batch_deadline = strtod(argv[6], &end);
    double now = monotonic_seconds();
    if (!end || *end || !isfinite(batch_deadline) || now < 0 ||
        batch_deadline <= now || batch_deadline > now + 600.1) return 124;
    const char *probe = argv[1];
    const char *case_id = argv[2];
    int attempt = 0;
    if (probe[0] != '/' || argv[4][0] != '/' || argv[5][0] != '/' ||
        !valid_case(case_id) || !valid_attempt(argv[3], &attempt)) {
        fprintf(stderr, "invalid closed controller argument\n");
        return 64;
    }

    struct stat probe_status;
    struct stat evidence_status;
    struct stat work_status;
    if (lstat(probe, &probe_status) != 0 || !S_ISREG(probe_status.st_mode) ||
        (probe_status.st_mode & 0022) != 0 ||
        lstat(argv[4], &evidence_status) != 0 || !S_ISDIR(evidence_status.st_mode) ||
        (evidence_status.st_mode & 0077) != 0 ||
        lstat(argv[5], &work_status) != 0 || !S_ISDIR(work_status.st_mode) ||
        (work_status.st_mode & 0077) != 0 || probe_status.st_uid != geteuid() ||
        evidence_status.st_uid != geteuid() || work_status.st_uid != geteuid()) {
        fprintf(stderr, "unsafe controller path\n");
        return 65;
    }

    char stdout_path[4096];
    char stderr_path[4096];
    int stdout_file = open_output(argv[4], case_id, attempt, "stdout", stdout_path);
    int stderr_file = open_output(argv[4], case_id, attempt, "stderr", stderr_path);
    if (stdout_file < 0 || stderr_file < 0) {
        fprintf(stderr, "cannot create bounded output\n");
        if (stdout_file >= 0) close(stdout_file);
        if (stderr_file >= 0) close(stderr_file);
        return 65;
    }

    int stdout_pipe[2];
    int stderr_pipe[2];
    if (pipe(stdout_pipe) != 0 || pipe(stderr_pipe) != 0) {
        fprintf(stderr, "cannot create pipes\n");
        return 70;
    }
    struct sigaction action;
    memset(&action, 0, sizeof(action));
    action.sa_handler = on_cancel;
    sigemptyset(&action.sa_mask);
    (void)sigaction(SIGINT, &action, NULL);
    (void)sigaction(SIGTERM, &action, NULL);

    if (make_nonblocking(stdout_pipe[0]) != 0 || make_nonblocking(stderr_pipe[0]) != 0) {
        return 70;
    }
    pid_t child;
    if (spawn_probe(&child, probe, case_id, argv[5], stdout_pipe[1], stderr_pipe[1]) != 0) return 70;
    (void)close(stdout_pipe[1]);
    (void)close(stderr_pipe[1]);
    struct stream_state streams[2] = {
        {stdout_pipe[0], stdout_file, 0, 1, 0},
        {stderr_pipe[0], stderr_file, 0, 1, 0},
    };

    int status = 0;
    int reaped = 0;
    int timed_out = 0;
    int cancelled = 0;
    int timeout_seconds = strcmp(case_id, "R0A_DEADLINE") == 0
                              ? DEADLINE_TIMEOUT_SECONDS
                              : NORMAL_TIMEOUT_SECONDS;
    double deadline = monotonic_seconds() + timeout_seconds;
    int batch_limited = batch_deadline < deadline;
    if (batch_limited) deadline = batch_deadline;
    int io_error = 0;
    while (!reaped && !timed_out && !cancelled && !streams[0].overflow &&
           !streams[1].overflow) {
        if (wait_with_deadline(child, &status, &reaped, deadline, streams, 0) != 0) { io_error = 1; break; }
        if (!reaped && monotonic_seconds() >= deadline) timed_out = 1;
        if (cancellation_signal != 0) cancelled = 1;
    }
    int cleanup_confirmed = 1;
    if (!reaped && terminate_and_reap(child, &status, &reaped, streams) != 0) {
        cleanup_confirmed = 0;
    }
    for (size_t index = 0; index < 2; index++) {
        while (reaped && streams[index].open && !streams[index].overflow) {
            size_t before = streams[index].bytes;
            if (drain_stream(&streams[index]) != 0) { io_error = 1; break; }
            if (streams[index].open && streams[index].bytes == before) break;
        }
        (void)close(streams[index].destination);
    }

    if (reaped && strcmp(case_id, "R0A_FSIZE") == 0) {
        const char *names[] = {"r0a-fsize-one.bin", "r0a-fsize-two.bin"};
        for (int i = 0; i < 2; i++) {
            char path[4096];
            struct stat info;
            int n = snprintf(path, sizeof(path), "%s/%s", argv[5], names[i]);
            int fd = n > 0 && (size_t)n < sizeof(path) ? open(path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK) : -1;
            long long size = fd >= 0 && fstat(fd, &info) == 0 && S_ISREG(info.st_mode) ? (long long)info.st_size : -1;
            if (fd >= 0) close(fd);
            printf("fsize_%d=%lld\n", i + 1, size);
        }
    }

    int exit_code = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
    int signal_number = WIFSIGNALED(status) ? WTERMSIG(status) : 0;
    printf("exit_code=%d\nsignal=%d\ntimed_out=%d\ncancelled=%d\n"
           "cleanup=%s\nstdout_bytes=%zu\nstderr_bytes=%zu\n"
           "stdout_overflow=%d\nstderr_overflow=%d\n",
           exit_code, signal_number, timed_out, cancelled,
           cleanup_confirmed && reaped ? "CONFIRMED" : "UNCONFIRMED", streams[0].bytes,
           streams[1].bytes, streams[0].overflow, streams[1].overflow);
    if (!cleanup_confirmed || !reaped) {
        fflush(stdout);
        /* Stop admission but retain ownership until eventual exit, not a stale PID. */
        while (waitpid(child, &status, 0) < 0 && errno == EINTR) {}
        return 125;
    }
    if (cancelled) return 130;
    if (io_error) return 74;
    if (streams[0].overflow || streams[1].overflow) return 74;
    if (timed_out) return !batch_limited && strcmp(case_id, "R0A_DEADLINE") == 0 ? 0 : 124;
    return exit_code == 0 ? 0 : exit_code;
}
