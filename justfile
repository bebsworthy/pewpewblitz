default: run

# Build both isolated configurations, then run the dedicated server and client
# together. Ctrl-C or closing the client shuts down the server as well.
run:
    #!/usr/bin/env bash
    set -euo pipefail

    cargo build --locked --no-default-features --features server --bin brawler-server
    cargo build --locked --no-default-features --features client --bin brawler-client

    server_pid=""
    cleanup() {
        if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
            # Background jobs inherit the shell's ignored SIGINT disposition,
            # so use SIGTERM for reliable launcher cleanup.
            kill -TERM "$server_pid" 2>/dev/null || true
            wait "$server_pid" 2>/dev/null || true
        fi
    }
    trap cleanup INT TERM EXIT

    target/debug/brawler-server &
    server_pid=$!
    target/debug/brawler-client
