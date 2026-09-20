BEGIN {
    FS = mode == "probe" ? "=" : "\t"
    expected["R0A_BASELINE"] = 1
    expected["R0A_ALLOC_CONTROL"] = 1
    expected["R0A_SMALL_CONTROL"] = 1
    expected["R0A_AS_SMALL"] = 1
    expected["R0A_AS_MALLOC"] = 1
    expected["R0A_AS_MMAP"] = 1
    expected["R0A_AS_EXEC"] = 1
    expected["R0A_AS_FIXED"] = 1
    expected["R0A_VM_REGIONS"] = 1
    expected["R0A_FSIZE"] = 1
    expected["R0A_NOFILE"] = 1
    expected["R0A_DEADLINE"] = 1
    failures = 0
    keys="origin probe_result as_soft as_hard as_error fsize_soft fsize_hard fsize_error nofile_soft nofile_hard nofile_error pid vm_regions virtual_bytes vm_complete malloc_success malloc_errno mmap_success mmap_errno errno touched_bytes target allocation_success expected_target inherited_soft inherited_hard raise_result raise_errno allocation_errno set_result regions complete protection_0_bytes protection_1_bytes protection_2_bytes protection_3_bytes protection_4_bytes protection_5_bytes protection_6_bytes protection_7_bytes first_bytes first_errno second_bytes second_errno aggregate_bytes saw_sigxfsz controller_first_bytes controller_second_bytes initially_open successful_dups final_errno deadline_probe_started set_limit_error get_limit_error exec_error inventory_error handler_error precondition_error"
    n=split(keys, names, " ")
    for (i=1; i<=n; i++) allowed_field[names[i]]=1
}

function require_fields(names,    count, parts, i) {
    count=split(names, parts, " ")
    for (i=1; i<=count; i++) {
        if (!(parts[i] in observation) || observation[parts[i]] !~ /^-?[0-9]+$/ ||
            length(observation[parts[i]]) > 20) fail("missing or invalid observation " parts[i])
    }
}

function check_probe(    conclusion, expected_result, i, total) {
    if (case_id == "R0A_DEADLINE") {
        if (result == "OBSERVED_EXPECTED" && observation["deadline_probe_started"] != "1")
            fail("deadline probe never confirmed startup")
        return
    }
    conclusion=observation["probe_result"]
    if (conclusion != "EXPECTED" && conclusion != "COUNTEREXAMPLE" && conclusion != "INCONCLUSIVE")
        fail("missing or invalid probe result")
    if ((conclusion == "INCONCLUSIVE" ? "INCONCLUSIVE" : "OBSERVED_" conclusion) != result)
        fail("probe result contradicts summary")
    if (conclusion == "INCONCLUSIVE") return
    expected_result=0
    if (case_id == "R0A_BASELINE") {
        require_fields("as_soft as_hard fsize_soft fsize_hard nofile_soft nofile_hard pid vm_regions virtual_bytes vm_complete")
        expected_result=observation["vm_complete"] == 1
    } else if (case_id == "R0A_ALLOC_CONTROL") {
        require_fields("malloc_success malloc_errno mmap_success mmap_errno")
        expected_result=observation["malloc_success"] == 1 && observation["mmap_success"] == 1
    } else if (case_id == "R0A_SMALL_CONTROL" || case_id == "R0A_AS_SMALL") {
        require_fields("malloc_success errno touched_bytes")
        if (case_id == "R0A_AS_SMALL") require_fields("target")
        expected_result=observation["malloc_success"] == 1 && observation["touched_bytes"] == 8388608
    } else if (case_id == "R0A_AS_MALLOC" || case_id == "R0A_AS_MMAP") {
        require_fields("target allocation_success errno")
        if (observation["allocation_success"] == 0 && observation["errno"] != 12)
            fail("allocation refusal is not ENOMEM")
        expected_result=observation["allocation_success"] == 0 && observation["errno"] == 12
    } else if (case_id == "R0A_AS_EXEC") {
        require_fields("expected_target inherited_soft inherited_hard raise_result raise_errno allocation_success allocation_errno")
        if ((observation["raise_result"] == -1 && observation["raise_errno"] != 1) ||
            (observation["allocation_success"] == 0 && observation["allocation_errno"] != 12))
            fail("unexpected exec resource error")
        expected_result=observation["expected_target"] == observation["inherited_soft"] &&
            observation["expected_target"] == observation["inherited_hard"] &&
            observation["raise_result"] == -1 && observation["raise_errno"] == 1 &&
            observation["allocation_success"] == 0 && observation["allocation_errno"] == 12
    } else if (case_id == "R0A_AS_FIXED") {
        require_fields("target set_result errno")
        expected_result=1 # This case observes the return, not a promised deployment limit.
    } else if (case_id == "R0A_VM_REGIONS") {
        require_fields("regions virtual_bytes complete protection_0_bytes protection_1_bytes protection_2_bytes protection_3_bytes protection_4_bytes protection_5_bytes protection_6_bytes protection_7_bytes")
        for (i=0; i<8; i++) total+=observation["protection_" i "_bytes"]
        expected_result=observation["complete"] == 1 && total == observation["virtual_bytes"]
    } else if (case_id == "R0A_FSIZE") {
        require_fields("first_bytes first_errno second_bytes second_errno aggregate_bytes saw_sigxfsz controller_first_bytes controller_second_bytes")
        if (observation["first_bytes"] != observation["controller_first_bytes"] ||
            observation["second_bytes"] != observation["controller_second_bytes"])
            fail("controller file size contradicts probe")
        if (observation["aggregate_bytes"] != observation["first_bytes"] + observation["second_bytes"])
            fail("aggregate size contradicts files")
        expected_result=observation["first_bytes"] >= 0 && observation["second_bytes"] >= 0 &&
            observation["first_bytes"] <= 65536 && observation["second_bytes"] <= 65536 && observation["aggregate_bytes"] > 65536
    } else if (case_id == "R0A_NOFILE") {
        require_fields("initially_open successful_dups final_errno")
        expected_result=observation["initially_open"] <= 16 && observation["successful_dups"] > 0 &&
            observation["successful_dups"] <= 32 && observation["final_errno"] == 24
    } else fail("unknown probe case")
    if ((conclusion == "EXPECTED") != expected_result) fail("observation fields contradict result")
}

mode == "probe" {
    if (FNR > 128 || length($0) > 256 || NF != 2 || !($1 in allowed_field) ||
        seen_field[$1]++) fail("malformed or duplicate observation")
    observation[$1]=$2
    next
}

function fail(message) {
    print "evidence error: " message > "/dev/stderr"
    failures += 1
}

FILENAME == ARGV[1] && FNR == 1 {
    if ($0 != "case_id\tdescription") fail("invalid cases header")
    next
}

FILENAME == ARGV[1] {
    if (NF != 2 || !($1 in expected) || seen_case[$1]++) {
        fail("invalid, unknown, or duplicate case row")
    }
    next
}

FILENAME == ARGV[2] && FNR == 1 {
    if ($0 != "case_id\tattempt\tresult\texit_code\tsignal\ttimed_out\tcleanup\tstdout_sha256\tstderr_sha256\tstdout_bytes\tstderr_bytes") {
        fail("invalid summary header")
    }
    next
}

FILENAME == ARGV[2] {
    if (NF != 11 || !($1 in expected) || $2 !~ /^[123]$/) {
        fail("invalid summary identity")
        next
    }
    key = $1 SUBSEP $2
    if (seen_attempt[key]++) fail("duplicate case attempt")
    if ($3 != "OBSERVED_EXPECTED" && $3 != "OBSERVED_COUNTEREXAMPLE" &&
        $3 != "INCONCLUSIVE" && $3 != "NOT_RUN") {
        fail("unknown result")
    }
    if ($4 !~ /^-?[0-9]+$/ || $5 !~ /^[0-9]+$/ || $6 !~ /^[01]$/) {
        fail("invalid process result")
    }
    if ($7 != "CONFIRMED") fail("cleanup is not confirmed")
    if (length($8) != 64 || $8 !~ /^[0-9a-f]+$/ ||
        length($9) != 64 || $9 !~ /^[0-9a-f]+$/) {
        fail("invalid output digest")
    }
    if ($10 !~ /^[0-9]+$/ || $11 !~ /^[0-9]+$/ || $10 > 32768 || $11 > 32768) {
        fail("invalid output byte count")
    }
    if (($3 == "OBSERVED_EXPECTED" || $3 == "OBSERVED_COUNTEREXAMPLE") &&
        $1 != "R0A_DEADLINE" && ($4 != 0 || $5 != 0 || $6 != 0)) {
        fail("normal observation does not have a clean exit")
    }
    if ($3 == "OBSERVED_EXPECTED" && $1 == "R0A_DEADLINE" &&
        ($6 != 1 || ($5 != 15 && $5 != 9))) {
        fail("deadline observation lacks bounded termination")
    }
    result_count[$3] += 1
    results[$1 SUBSEP $2]=$3
}

END {
    if (mode == "probe") { check_probe(); exit failures != 0 }
    for (case_id in expected) {
        if (seen_case[case_id] != 1) fail("missing canonical case " case_id)
        for (attempt = 1; attempt <= 3; attempt++) {
            if (seen_attempt[case_id SUBSEP attempt] != 1) {
                fail("missing attempt for " case_id)
            }
        }
    }
    for (attempt=1; attempt<=3; attempt++) {
        for (case_id in expected) {
            if (case_id ~ /^R0A_AS_(SMALL|MALLOC|MMAP|EXEC)$/ &&
                results[case_id SUBSEP attempt] == "OBSERVED_EXPECTED" &&
                (results["R0A_ALLOC_CONTROL" SUBSEP attempt] != "OBSERVED_EXPECTED" ||
                 results["R0A_SMALL_CONTROL" SUBSEP attempt] != "OBSERVED_EXPECTED"))
                fail("AS observation lacks successful paired controls")
        }
    }
    if (failures != 0) exit 1
    printf "evidence_structure=VALID expected=%d counterexamples=%d inconclusive=%d not_run=%d\n",
        result_count["OBSERVED_EXPECTED"] + 0,
        result_count["OBSERVED_COUNTEREXAMPLE"] + 0,
        result_count["INCONCLUSIVE"] + 0,
        result_count["NOT_RUN"] + 0
}
