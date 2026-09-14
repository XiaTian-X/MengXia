#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-011.sh developer|formal [component]" >&2; exit 64 ;;
esac
native=0
component=0
case "${2-}" in
    "") ;;
    component) component=1 ;;
    native-component) component=1; native=1 ;;
    *) echo "usage: scripts/verify-task-011.sh developer|formal [component]" >&2; exit 64 ;;
esac
test "$#" -le 2

. scripts/ci-evidence.sh

run() {
    test_id=$1
    shift
    test "$#" -gt 0
    "$@"
    ci_result "$test_id"
}

proto_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation plugin_descriptor_source_and_authority_graph_are_exact
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation protocol_golden_messages_are_byte_exact
    if [ "$native" -eq 1 ] || [ "$component" -eq 0 ]; then
        ./scripts/verify-proto-artifacts.sh
    fi
}

wire_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation closed_wire_scanner_rejects_noncanonical_and_unknown_inputs
}

authority_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation plugin_descriptor_source_and_authority_graph_are_exact
    cargo test --locked --offline -p mengxia-testkit --test architecture
}

bounds_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation limits_are_tightening_only_and_checked
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation closed_wire_scanner_rejects_noncanonical_and_unknown_inputs
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation handshake_ping_and_shutdown_are_correlated_and_joined
}

queue_check() {
    cargo test --locked --offline -p mengxia-plugin-host inbound_and_outbound_frame_queues_are_lazy_bounded_and_restore_capacity
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation admission_is_bounded_before_stream_ownership
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation one_in_flight_request_applies_backpressure_without_a_side_queue
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation handshake_ping_and_shutdown_are_correlated_and_joined
}

hostile_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation hostile_plugin_matrix_is_fail_closed_and_every_child_is_reaped -- --test-threads=1
}

lifecycle_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation handshake_ping_and_shutdown_are_correlated_and_joined
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation dropping_unpolled_driver_releases_streams_and_admission
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation driver_poll_panic_is_caught_and_releases_admission
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation dropping_an_admitted_ping_cancels_the_session_immediately
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation dropping_shutdown_while_a_request_is_active_cancels_instead_of_wedging_closing
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation unsolicited_response_is_detected_while_active_and_closes_the_session
    hostile_check
}

deadline_check() {
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation silent_handshake_uses_one_absolute_deadline_and_releases_admission
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation wrong_challenge_and_timeout_fail_closed_without_disclosure
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation pending_and_partial_handshake_writes_and_flushes_are_deadline_bounded
    cargo test --locked --offline -p mengxia-plugin-host operation_success_is_rechecked_before_deadline_or_cancellation_commit
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation request_and_shutdown_writes_and_flushes_share_their_absolute_deadlines
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation dropping_an_admitted_ping_cancels_the_session_immediately
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation dropping_shutdown_while_a_request_is_active_cancels_instead_of_wedging_closing
    hostile_check
}

supply_check() {
    test "$(shasum -a 256 crates/mengxia-testkit/tests/fixtures/task_011/Cargo.task-010.lock | awk '{print $1}')" = \
        302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e
    cargo metadata --locked --offline --format-version 1 --no-deps >/dev/null
    cargo tree --locked --offline -p mengxia-plugin-host >/dev/null
    cargo tree --locked --offline -p mengxia-plugin-proto --edges normal,build >/dev/null
    cargo test --locked --offline -p mengxia-testkit --test task_011_foundation task_011_dependency_delta_adds_no_third_party_inventory
    ci_supply
}

run TEST-PROTO-011 proto_check
run TEST-WIRE-011 wire_check
run TEST-AUTHORITY-011 authority_check
run TEST-BOUNDS-011 bounds_check
run TEST-QUEUE-011 queue_check
run TEST-STDERR-011 hostile_check
run TEST-DEADLINE-011 deadline_check
run TEST-LIFECYCLE-011 lifecycle_check
run TEST-HOSTILE-011 hostile_check
run TEST-ARCH-011 cargo test --locked --offline -p mengxia-testkit --test architecture
run TEST-SUPPLY-011 supply_check
run TEST-DOC-011 cargo test --locked --offline -p mengxia-testkit --test document_traceability

if [ "$component" -eq 0 ]; then
    cargo fmt --all -- --check
    cargo clippy --locked --offline --workspace --all-targets --all-features -- -D warnings
    cargo test --locked --offline --workspace --all-targets --all-features
    git diff --check
fi
