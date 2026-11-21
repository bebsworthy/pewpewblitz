# Outcome
A freshly installed release client, on Android or desktop, connects to the public server by its DNS name with no typing, and the same name serves the HTTPS token endpoint, health check and privacy pages.

# Found (2026-11-21)
- **The client already resolves host names.** Server Select resolves `HOST[:PORT]` off-thread with `to_socket_addrs` (`packages/brawler-client/src/flow/connection.rs:157-192`), so no DNS code is needed.
- **Accounts survive an IP change.** They're bound per logical server ID, and the address list per logical server can grow (`connection_persistence.rs:238-275`). A record pointing to a new IP keeps players' accounts.
- **The prefill is build-time.** Android uses `BRAWLER_ANDROID_SERVER` via `just android-build server=HOST:PORT` (`packages/brawler-android/src/lib.rs:13`). Desktop uses `--server` (`packages/brawler-client/src/bin/client.rs:57`).
- **Android always opens at Server Select.** Startup connects are skipped there because of the Android 17 local-network permission prompt (`flow/connection.rs:281`, `MainActivity.java`). For a public internet server that prompt is unnecessary friction.
- **One name, two services:** after BRL-z277 the name serves UDP 5000 (game) and HTTPS 443 (token service, `/healthz`, plus the BRL-tysh policy and deletion pages).

# Decisions (owner-approved 2026-11-21)
1. A release build's default target is `<domain>:5000`, with the token URL derived as `https://<domain>/v1/lobby-token`. The default is shown as a built-in "PewPew Blitz" server entry, not just a prefilled text field. A custom address stays available.
2. The Android local-network permission is requested only when the chosen target is a private or link-local address. The public default connects directly at startup, like desktop.
3. DNS: an A record (plus AAAA once BRL-h018 decides IPv6) with **TTL 300 s**, so an IP move during an incident (BRL-h018 runbook) propagates quickly.
4. A failed DNS lookup shows the existing connection-failure state with Retry; no offline mode.

# Scope
Decisions 1–4, plus tests for the default-entry selection and the permission-only-for-LAN rule.

# Acceptance
A release APK and desktop build connect to the domain without input; changing the A record moves new connections without a rebuild; no local-network prompt appears for the public server.

# Verification
`just test`, then a manual Android run on the real domain (recorded).
