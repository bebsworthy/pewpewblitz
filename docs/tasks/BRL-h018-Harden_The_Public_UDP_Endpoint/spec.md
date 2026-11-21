# Outcome
The public host exposes only the game UDP port, HTTPS on 443 and restricted SSH, and the owner knows what happens under a flood and what to do about it.

# Owner context
Open public release (q-tzc8kxb) on a Scaleway DEV1-M (BRL-kc93). Public ports are UDP 5000 (game) and TCP 443 (token service and `/healthz` via Caddy, BRL-z277 and BRL-71r3).

# Found (2026-11-21)
## Scaleway
- **Security groups** are the instance firewall. The **default group allows all inbound traffic** (and drops outbound SMTP). Console-created groups are stateful. ([use security groups](https://www.scaleway.com/en/docs/instances/how-to/use-security-groups/))
- Scaleway's **DDoS guidance points to Load Balancers and Edge Services (WAF)**, which protect HTTP(S), not raw UDP game traffic ([security baseline](https://www.scaleway.com/en/docs/use-cases/security/security-baseline/)). No documented network-level DDoS mitigation for instance public IPs was found; to confirm with Scaleway support.
- Scaleway locks instances that *send* DDoS traffic ([preventing outgoing DDoS](https://www.scaleway.com/en/docs/instances/reference-content/preventing-outgoing-ddos/)). The server must not reflect or amplify: no open resolvers, NTP or memcached on the host.

## Application defences already in place
- Pre-auth per-source allowances (8 connection requests and 9 KiB, plus 16 other datagrams and 6 KiB per 10 s; `PUBLIC_PREAUTH_*` in `packages/brawler-routing/src/limits.rs`).
- Bounded routes, queues and capabilities.
- Malformed and oversized envelopes dropped before allocation (docs/14 "Failure and security requirements").
- **No amplification:** pre-auth replies are never larger than requests (to verify with a test).

## Exposure
A volumetric UDP flood larger than the DEV1-M's 300 Mbps link would take the server offline regardless of application limits. Application limits protect CPU and memory, not bandwidth.

# Decisions (owner-approved 2026-11-21)
1. **Security group** (stateful, inbound default **drop**): allow UDP 5000 and TCP 443 from anywhere; TCP 80 only if Caddy needs it for ACME HTTP-01 (prefer TLS-ALPN-01 on 443 and keep 80 closed); TCP 22 only from the owner's IPs, or keep SSH closed and use the Scaleway console. ICMP allowed. Define it as code (Scaleway CLI or Terraform snippet) in `deploy/` and document it in `docs/19-server-operations.md`.
2. **SSH:** key-only, no password and no root login; unattended security upgrades on the host OS.
3. **Accept volumetric-flood risk at launch.** No UDP scrubbing front at launch: the options are specialised game-traffic proxies, and they're out of budget and scope. The application limits stay the defence for everything below link saturation.
4. **Incident runbook** in docs/19: how to spot a flood (ingress summary counters, `/healthz` 503, bandwidth graph); how to move to a new flexible IP and update DNS (BRL-3he3 TTL ≤ 300 s); when to open a Scaleway support ticket; and filing a follow-up ticket for a scrubbing front if floods recur.
5. **Amplification test:** a routing test asserting that for every pre-auth input the supervisor and lobby never send more bytes than they received from that source in the window.

# Acceptance
- An external port scan of the live host shows only 443/tcp and 5000/udp (plus SSH if kept), recorded here.
- The amplification test passes.
- The runbook exists, and the security-group definition is checked in.
- Scaleway support's answer on instance DDoS protection is recorded here.

# Verification
`cargo test -p brawler-routing`, the port scan output and the runbook review.
