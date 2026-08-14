#!/usr/bin/env bash
set -euo pipefail

runs="${BRAWLER_NETWORK_PROFILE_RUNS:-3}"
base_port="${BRAWLER_NETWORK_PROFILE_BASE_PORT:-5100}"
if ! [[ "$runs" =~ ^[1-9][0-9]*$ ]]; then
    printf 'brawler combat profiles: BRAWLER_NETWORK_PROFILE_RUNS must be a positive integer\n' >&2
    exit 2
fi
if ! [[ "$base_port" =~ ^[1-9][0-9]*$ ]] || ((base_port > 65000)); then
    printf 'brawler combat profiles: BRAWLER_NETWORK_PROFILE_BASE_PORT must be between 1 and 65000\n' >&2
    exit 2
fi

report_dir="$(mktemp -d "${TMPDIR:-/tmp}/brawler-combat-profiles.XXXXXX")"
results_file="$report_dir/results.tsv"
cleanup() {
    rm -rf "$report_dir"
}
trap cleanup EXIT

run_number=0
for profile in local typical adverse; do
    for repeat in $(seq 1 "$runs"); do
        run_number=$((run_number + 1))
        run_id="${profile}-${repeat}-$(date +%s)"
        port=$((base_port + run_number - 1))
        if ((port > 65535)); then
            printf 'brawler combat profiles: run port exceeds 65535\n' >&2
            exit 2
        fi
        report_file="$report_dir/$run_id.report"
        log_file="$report_dir/$run_id.log"
        if ! BRAWLER_NETWORK_HEADLESS=1 \
            BRAWLER_NETWORK_ASSERT_COMBAT=1 \
            BRAWLER_NETWORK_PROFILE="$profile" \
            BRAWLER_NETWORK_RUN_ID="$run_id" \
            BRAWLER_NETWORK_COMBAT_REPORT_FILE="$report_file" \
            BRAWLER_NETWORK_ADDR="127.0.0.1:$port" \
            ./scripts/network.sh >"$log_file" 2>&1; then
            cat "$log_file" >&2
            printf 'brawler combat profiles: failed run %s\n' "$run_id" >&2
            exit 1
        fi
        server_ms="$(awk -F= '/^server_elapsed_ms=/{print $2}' "$report_file")"
        client_one_ms="$(awk -F= '/^client_one=client_elapsed_ms=/{print $3}' "$report_file")"
        client_two_ms="$(awk -F= '/^client_two=client_elapsed_ms=/{print $3}' "$report_file")"
        cue_one_values="$(awk -F= '/^fire_to_cue_client_one_us=/{print $2}' "$report_file" | sort -n)"
        cue_two_values="$(awk -F= '/^fire_to_cue_client_two_us=/{print $2}' "$report_file" | sort -n)"
        cue_one_us="$(printf '%s\n' "$cue_one_values" | awk 'NF {values[++count]=$1} END {if (!count) exit 1; print values[int((count + 1) / 2)]}')"
        cue_two_us="$(printf '%s\n' "$cue_two_values" | awk 'NF {values[++count]=$1} END {if (!count) exit 1; print values[int((count + 1) / 2)]}')"
        state_converged="$(awk -F= '/^state_converged=/{print $2}' "$report_file")"
        cue_converged="$(awk -F= '/^cue_converged=/{print $2}' "$report_file")"
        if [[ -z "$server_ms" || -z "$client_one_ms" || -z "$client_two_ms" \
            || -z "$cue_one_us" || -z "$cue_two_us" || "$state_converged" != "1" \
            || "$cue_converged" != "1" ]]; then
            cat "$report_file" >&2
            printf 'brawler combat profiles: incomplete report for run %s\n' "$run_id" >&2
            exit 1
        fi
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$profile" "$run_id" "$server_ms" "$client_one_ms" "$client_two_ms" \
            "$cue_one_us" "$cue_two_us" >>"$results_file"
        printf 'run_id=%s profile=%s server_ms=%s client1_ms=%s client2_ms=%s cue1_us=%s cue2_us=%s\n' \
            "$run_id" "$profile" "$server_ms" "$client_one_ms" "$client_two_ms" "$cue_one_us" "$cue_two_us"
    done
done

for profile in local typical adverse; do
    server_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $3}' "$results_file" | sort -n)"
    client_one_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $4}' "$results_file" | sort -n)"
    client_two_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $5}' "$results_file" | sort -n)"
    cue_one_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $6}' "$results_file" | sort -n)"
    cue_two_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $7}' "$results_file" | sort -n)"
    stats="$(printf '%s\n' "$server_values" | awk '
        NF { values[++count] = $1 }
        END {
            if (count == 0) exit 1
            median = values[int((count + 1) / 2)]
            p95_rank = int((count * 95 + 99) / 100)
            if (p95_rank < 1) p95_rank = 1
            print count, median, values[p95_rank]
        }')"
    read -r count server_median server_p95 <<<"$stats"
    client_one_median="$(printf '%s\n' "$client_one_values" | awk 'NF {values[++count]=$1} END {print values[int((count + 1) / 2)]}')"
    client_two_median="$(printf '%s\n' "$client_two_values" | awk 'NF {values[++count]=$1} END {print values[int((count + 1) / 2)]}')"
    cue_one_stats="$(printf '%s\n' "$cue_one_values" | awk '
        NF { values[++count] = $1 }
        END {
            if (count == 0) exit 1
            p95_rank = int((count * 95 + 99) / 100)
            if (p95_rank < 1) p95_rank = 1
            print values[int((count + 1) / 2)], values[p95_rank]
        }')"
    cue_two_stats="$(printf '%s\n' "$cue_two_values" | awk '
        NF { values[++count] = $1 }
        END {
            if (count == 0) exit 1
            p95_rank = int((count * 95 + 99) / 100)
            if (p95_rank < 1) p95_rank = 1
            print values[int((count + 1) / 2)], values[p95_rank]
        }')"
    read -r cue_one_median cue_one_p95 <<<"$cue_one_stats"
    read -r cue_two_median cue_two_p95 <<<"$cue_two_stats"
    printf 'summary profile=%s runs=%s server_ms_median=%s server_ms_p95=%s client1_ms_median=%s client2_ms_median=%s fire_to_cue_client1_us_median=%s fire_to_cue_client1_us_p95=%s fire_to_cue_client2_us_median=%s fire_to_cue_client2_us_p95=%s\n' \
        "$profile" "$count" "$server_median" "$server_p95" "$client_one_median" "$client_two_median" \
        "$cue_one_median" "$cue_one_p95" "$cue_two_median" "$cue_two_p95"
done
