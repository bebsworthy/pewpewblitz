# Scope

Split combat visual-state work into focused systems for presentation facts/overheads, status visuals, dash trails, and aim previews.

# Acceptance

- Each system owns one lifecycle and uses focused queries/change gates.
- Dash trails have direct owner linkage rather than all-trails-per-fighter scans.
- Ordering exists only where one system produces a same-frame fact for another.
- Add a shared facts cache only if measurement justifies it.
- Health/ammo/name, concealment/reveal, dash, and previews retain native visual parity.
- Client Clippy, tests, diagnostics, and performance evidence pass.

# Constraints

Do not combine authority with presentation or create a general presentation framework.
