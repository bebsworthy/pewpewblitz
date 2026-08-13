default: run

# Build both isolated configurations, then run the dedicated server and client
# together. Ctrl-C, closing the client, or a server failure shuts down the
# other process and determines the launcher status.
run:
    #!/usr/bin/env bash
    set -euo pipefail

    cargo build --locked --no-default-features --features server --bin brawler-server
    cargo build --locked --no-default-features --features client --bin brawler-client

    server_pid=""
    client_pid=""

    job_is_running() {
        jobs -pr | grep -qx "$1"
    }

    cleanup() {
        local status=$?

        if [[ -n "$client_pid" ]] && job_is_running "$client_pid"; then
            kill -TERM "$client_pid" 2>/dev/null || true
        fi
        if [[ -n "$server_pid" ]] && job_is_running "$server_pid"; then
            # Background jobs inherit the shell's ignored SIGINT disposition,
            # so use SIGTERM for reliable launcher cleanup.
            kill -TERM "$server_pid" 2>/dev/null || true
        fi
        wait "$client_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
        exit "$status"
    }
    trap cleanup EXIT
    trap 'exit 130' INT
    trap 'exit 143' TERM

    # Run through Cargo so CARGO_TARGET_DIR and Cargo configuration determine
    # the executable location instead of assuming target/debug in the repo.
    (exec cargo run --locked --no-default-features --features server --bin brawler-server) &
    server_pid=$!
    (exec cargo run --locked --no-default-features --features client --bin brawler-client) &
    client_pid=$!

    while :; do
        # jobs -pr distinguishes completed background jobs from live ones;
        # kill -0 alone can still report a completed, unreaped child as alive.
        if ! job_is_running "$server_pid"; then
            if wait "$server_pid"; then
                server_status=0
            else
                server_status=$?
            fi
            printf 'brawler launcher: server exited with status %s; stopping client\n' "$server_status" >&2
            kill -TERM "$client_pid" 2>/dev/null || true
            wait "$client_pid" 2>/dev/null || true
            exit "$server_status"
        fi

        if ! job_is_running "$client_pid"; then
            if wait "$client_pid"; then
                client_status=0
            else
                client_status=$?
            fi
            exit "$client_status"
        fi

        sleep 0.1
    done
