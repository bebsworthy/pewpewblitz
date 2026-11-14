# Outcome

Casting Demolition Strike or Big Blob gives immediate audio feedback on the press, like the other ultimates since BRL-0172.

# Context

- BRL-0172 plays the activation sound on the input frame only for ultimates whose activation sound is a launch (`brawler_client::local_attack::presents_activation_sound_locally`).
- Demolition Strike's only sound is `ultimate-strike`, its explosion at the target (an area outcome that stays authoritative from `DemolitionStrikeActivated`). Big Blob has no activation sound at all.
- Both already get the local shoot motion, recoil, and launch flash from BRL-0142/BRL-0172.

# Scope

- Author (or choose existing) cast/launch sound assets and client-only audio catalog mappings for both kinds (`assets/catalogs/audio_profiles.ron`; not part of the gameplay fingerprint).
- Play them from the local `LocalAttackPresented` ultimate cast. For remote casters decide whether a cue-driven cast sound is wanted (Big Blob has no activation cue).
- Keep the strike explosion authoritative.

# Acceptance criteria

- A cast sound starts on the input frame for both kinds; no extra sound when the server activates; the strike explosion still plays once from authority.

# Verification

- Focused audio producer test; native playtest of both ultimates.

# Source

Deferred from BRL-0172 (2026-11-14).
