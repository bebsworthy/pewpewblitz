# Outcome
The first public server runs on a chosen European VPS whose size is justified by measurement and whose cost is known.

# Owner decisions
Europe, small VPS at about €5–20 per month, Hetzner, OVH or Scaleway class, with a dedicated IPv4 address and UDP allowed (q-twvv8jw, 2026-11-21). Open public release (q-tzc8kxb).

# Research (2026-11-21)
## Load (BRL-fdbz)
Memory is about 7 MiB for the supervisor, 30 MiB for the lobby and ≤ 47 MiB per match; CPU is the constraint. Four 3v3-with-bots matches peaked at 70% of one Apple M6 core. Estimate: roughly 3–6 concurrent matches per 2 shared vCPUs, to be confirmed on the host. The code currently caps a host at 4 matches and 32 lobby sessions (BRL-fdbz raises the caps).

## Options in budget
Third-party price trackers, November 2026, excluding VAT; check the provider console before buying.
- **Hetzner CAX11** (arm64 Ampere): 2 vCPU, 4 GB, 40 GB, about €5.99/month after the 2026 increases.
- **Hetzner CX23** (x86): 2 vCPU, 4 GB, 40 GB, about €5.49/month (it replaced the CX22).
- **OVHcloud VPS-1:** €6.49/month list price after the April 2026 change (2 vCPU, 4 GB per most trackers).
- Larger tiers such as Hetzner CAX21 (4 vCPU and 8 GB) also fit the budget; check prices at purchase.
- Hetzner and OVH both include basic network DDoS filtering (relevant to BRL-h018) and bill hourly, so a short benchmark rental costs cents.

Sources: [betterstack Hetzner review](https://betterstack.com/community/guides/web-servers/hetzner-cloud-review/), [eucloudcost Hetzner](https://www.eucloudcost.com/providers/hetzner/), [vpsfor.dev CX22/CX23](https://vpsfor.dev/posts/hetzner-cx22-pricing-2026/), [rscomputers Hetzner 2026 increase](https://rscomputers-ks.com/hetzner-price-increase-2026-move-servers/), [OVHcloud pricing changes](https://blog.ovhcloud.com/en/posts/pricing-evolution-of-public-cloud-bare-metal-and-vps-at-ovhcloud/), [trackvps OVH](https://trackvps.com/ovh-vps). Scaleway pricing was not found in this pass.

# Plan
1. Owner picks the provider and starting tier (question on this ticket).
2. Once BRL-jjwa produces an image, rent the chosen tier hourly, run the BRL-fdbz capacity procedure with full-length matches, and record matches per host at a p95 tick within budget.
3. Set `--max-matches` from that result with headroom, and record the region (for example Falkenstein, Nuremberg or Helsinki for Hetzner) and the monthly cost here.

# Acceptance
Provider, tier, architecture (feeds BRL-jjwa), region, measured match capacity and monthly cost are recorded here.

# Availability check (2026-11-21)
- **Hetzner console** (owner screenshot): CX23 €7.19/month, CAX11 €7.79/month and CX33 (4 vCPU, 8 GB) €10.79/month, all marked *not available*. These are higher than the trackers above; the console includes IPv4.
- **hetzner.com product pages** (checked the same day): every tier in Cost-Optimized (CX/CAX), Regular Performance (CPX) and General Purpose (CCX) shows "This product is currently unavailable. Please check back later." in FSN1, NBG1 and HEL1. Hetzner Cloud can't be ordered today, and no restock date is published.
- **OVHcloud VPS page (en-ie)**, prices ex. VAT, "from" (may assume a 6- or 12-month commitment; the page offers 2–3 months free for one), all x86 with unlimited traffic and daily backup included:

| Plan | vCores | RAM | Disk | Bandwidth | From |
|---|---|---|---|---|---|
| VPS-1 | 2 | 4 GB | 40 GB NVMe | 500 Mbps | €3.81/month |
| VPS-2 | 4 | 8 GB | 75 GB NVMe | 1 Gbps | €7.21/month |
| VPS-3 | 6 | 12 GB | 100 GB NVMe | 2 Gbps | €10.40/month |
| VPS-4 | 8 | 24 GB | 200 GB NVMe | 3 Gbps | €19.96/month |

- **Consequence:** the cheapest available 4-vCPU host in budget is OVH VPS-2, which also gives the CPU headroom the BRL-fdbz estimate calls for. OVH is x86-only, so BRL-jjwa would ship `linux/amd64` first. The question q-pkkbpsq was written before this check; most of its options can't be bought today.

# Alternatives after "no OVH" (owner, 2026-11-21)
The owner doesn't want OVH. Checked the same day on provider pages:
- **Scaleway** (scaleway.com pricing page; Paris, Amsterdam and Warsaw; hourly billing, no commitment):
  - DEV1-S: 2 vCPU, 2 GB, 200 Mbps, about €6.55/month (€0.00898/hour).
  - DEV1-M: 3 vCPU, 4 GB, 300 Mbps, about €14.74/month.
  - DEV1-L: 4 vCPU, 8 GB, about €31.27/month.
  - BASIC2-A2C-4G: 2 vCPU, 4 GB, about €16.79/month.
  - Still to confirm in the console: whether the public IPv4 is billed separately.
- **netcup** (netcup.com VPS page, Nuremberg; DDoS protection and traffic flat rate included; x86, with ARM64 plans too). Prices ex. VAT on a **12-month contract**; shorter terms cost more:
  - VPS 500 G12.5: 2 vCore, 4 GB, €8.26/month.
  - VPS 1000 G12.5: 4 vCore, 8 GB, €14.50/month.
- **Third-party list prices (eucloudcost, November 2026)** for 4 vCPU / 8 GB: UpCloud Starter €20/month; Vultr and DigitalOcean unconfirmed (about $24–48).

**Reading:**
- Memory needs are small (≤ 215 MiB for four matches), so even 2 GB is enough at first; CPU decides the size.
- Hourly billing (Scaleway) lets the BRL-fdbz capacity run happen on the real host before any commitment, and the instance can be resized later.
- netcup is cheaper per vCore but locks in a 12-month contract before the size is measured.

# Decision (owner, 2026-11-21)
**Scaleway DEV1-M**: 3 vCPU, 4 GB, 300 Mbps, about €14.74/month, billed hourly, x86_64. Hetzner couldn't be ordered and the owner ruled out OVH.

Remaining for this ticket:
- Pick the zone (Paris PAR, Amsterdam AMS or Warsaw WAW) for the player base; Paris or Amsterdam is the default for Western Europe.
- Confirm in the console whether a public IPv4 is billed separately, and the total monthly cost.
- Run the BRL-fdbz capacity procedure on a DEV1-M with the BRL-jjwa image and full-length matches. Set `--max-matches` from it, or move to DEV1-L (4 vCPU, 8 GB, about €31.27/month) if 3 vCPU can't hold the target.
- Record Scaleway's DDoS baseline for BRL-h018.
