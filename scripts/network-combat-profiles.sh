#!/usr/bin/env bash
set -euo pipefail

runs="${BRAWLER_NETWORK_PROFILE_RUNS:-3}"
base_port="${BRAWLER_NETWORK_PROFILE_BASE_PORT:-5100}"
presets="${BRAWLER_NETWORK_PROFILE_PRESETS:-1 2 3 4}"
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
    for preset in $presets; do
        if ! [[ "$preset" =~ ^[1-4]$ ]]; then
            printf 'brawler combat profiles: weapon presets must be integers from 1 through 4\n' >&2
            exit 2
        fi
        for repeat in $(seq 1 "$runs"); do
        run_number=$((run_number + 1))
        run_id="${profile}-preset-${preset}-${repeat}-$(date +%s)"
        port=$((base_port + run_number - 1))
        if ((port > 65535)); then
            printf 'brawler combat profiles: run port exceeds 65535\n' >&2
            exit 2
        fi
        report_file="$report_dir/$run_id.report"
        log_file="$report_dir/$run_id.log"
        if ! BRAWLER_NETWORK_HEADLESS=1 \
            BRAWLER_NETWORK_ASSERT_COMBAT=1 \
            BRAWLER_NETWORK_WEAPON_PRESET="$preset" \
            BRAWLER_NETWORK_WEAPON_PRESET_CLIENT_TWO="${BRAWLER_NETWORK_PROFILE_CLIENT_TWO_PRESET:-$preset}" \
            BRAWLER_NETWORK_COMBAT_TEST_PRESET="$preset" \
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
        server_cue_count="$(awk -F= '/^server_cue_count=/{print $2}' "$report_file")"
        client_one_cue_count="$(awk -F= '/^client_one_cue_count=/{print $2}' "$report_file")"
        client_two_cue_count="$(awk -F= '/^client_two_cue_count=/{print $2}' "$report_file")"
        state_one_median_us="$(awk -F= '/^state_convergence_client_one_us_median=/{print $2}' "$report_file")"
        state_one_p95_us="$(awk -F= '/^state_convergence_client_one_us_p95=/{print $2}' "$report_file")"
        state_two_median_us="$(awk -F= '/^state_convergence_client_two_us_median=/{print $2}' "$report_file")"
        state_two_p95_us="$(awk -F= '/^state_convergence_client_two_us_p95=/{print $2}' "$report_file")"
        state_converged="$(awk -F= '/^state_converged=/{print $2}' "$report_file")"
        cue_converged="$(awk -F= '/^cue_converged=/{print $2}' "$report_file")"
        tested_preset_id="$(awk -F= '/^tested_preset_id=/{print $2}' "$report_file")"
        tested_recipe_fingerprint="$(awk -F= '/^tested_recipe_fingerprint=/{print $2}' "$report_file")"
        tested_attacks="$(awk -F= '/^tested_accepted_attacks=/{print $2}' "$report_file")"
        tested_deliveries="$(awk -F= '/^tested_emitted_deliveries=/{print $2}' "$report_file")"
        if [[ -z "$server_ms" || -z "$client_one_ms" || -z "$client_two_ms" \
            || -z "$cue_one_us" || -z "$cue_two_us" || "$state_converged" != "1" \
            || -z "$state_one_median_us" || -z "$state_one_p95_us" \
            || -z "$state_two_median_us" || -z "$state_two_p95_us" \
            || -z "$server_cue_count" || "$server_cue_count" -lt 1 \
            || -z "$client_one_cue_count" || "$client_one_cue_count" -lt 1 \
            || -z "$client_two_cue_count" || "$client_two_cue_count" -lt 1 \
            || "$cue_converged" != "1" || "$tested_preset_id" != "$preset" \
            || -z "$tested_recipe_fingerprint" || "$tested_recipe_fingerprint" == "0" \
            || -z "$tested_attacks" || "$tested_attacks" -lt 1 \
            || -z "$tested_deliveries" || "$tested_deliveries" -lt 1 ]]; then
            cat "$report_file" >&2
            printf 'brawler combat profiles: incomplete report for run %s\n' "$run_id" >&2
            exit 1
        fi
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$profile" "$run_id" "$server_ms" "$client_one_ms" "$client_two_ms" \
            "$cue_one_us" "$cue_two_us" "$state_one_median_us" "$state_one_p95_us" \
            "$state_two_median_us" "$state_two_p95_us" >>"$results_file"
        printf 'run_id=%s profile=%s server_ms=%s client1_ms=%s client2_ms=%s cue1_us=%s cue2_us=%s state1_median_us=%s state1_p95_us=%s state2_median_us=%s state2_p95_us=%s server_cues=%s client1_cues=%s client2_cues=%s\n' \
            "$run_id" "$profile" "$server_ms" "$client_one_ms" "$client_two_ms" "$cue_one_us" "$cue_two_us" \
            "$state_one_median_us" "$state_one_p95_us" "$state_two_median_us" "$state_two_p95_us" \
            "$server_cue_count" "$client_one_cue_count" "$client_two_cue_count"
        done
    done
done

for profile in local typical adverse; do
    server_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $3}' "$results_file" | sort -n)"
    client_one_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $4}' "$results_file" | sort -n)"
    client_two_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $5}' "$results_file" | sort -n)"
    cue_one_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $6}' "$results_file" | sort -n)"
    cue_two_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $7}' "$results_file" | sort -n)"
    state_one_median_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $8}' "$results_file" | sort -n)"
    state_one_p95_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $9}' "$results_file" | sort -n)"
    state_two_median_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $10}' "$results_file" | sort -n)"
    state_two_p95_values="$(awk -F '\t' -v profile="$profile" '$1 == profile {print $11}' "$results_file" | sort -n)"
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
    state_one_stats="$(printf '%s\n' "$state_one_median_values" | awk 'NF {values[++count]=$1} END {print values[int((count + 1) / 2)]}')"
    state_one_p95_stats="$(printf '%s\n' "$state_one_p95_values" | awk 'NF {values[++count]=$1} END {print values[int((count + 1) / 2)]}')"
    state_two_stats="$(printf '%s\n' "$state_two_median_values" | awk 'NF {values[++count]=$1} END {print values[int((count + 1) / 2)]}')"
    state_two_p95_stats="$(printf '%s\n' "$state_two_p95_values" | awk 'NF {values[++count]=$1} END {print values[int((count + 1) / 2)]}')"
    printf 'summary profile=%s runs=%s server_ms_median=%s server_ms_p95=%s client1_ms_median=%s client2_ms_median=%s fire_to_cue_client1_us_median=%s fire_to_cue_client1_us_p95=%s fire_to_cue_client2_us_median=%s fire_to_cue_client2_us_p95=%s state_client1_us_median=%s state_client1_us_p95=%s state_client2_us_median=%s state_client2_us_p95=%s\n' \
        "$profile" "$count" "$server_median" "$server_p95" "$client_one_median" "$client_two_median" \
        "$cue_one_median" "$cue_one_p95" "$cue_two_median" "$cue_two_p95" \
        "$state_one_stats" "$state_one_p95_stats" "$state_two_stats" "$state_two_p95_stats"
done
