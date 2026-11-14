# Outcome

The game holds frame rate on target phones and never fails because of missing GPU features.

# Context

- `bevy_hanabi` (installed in `presentation_3d/combat/particles.rs`) needs compute/storage buffers, which rules out GLES-only devices.
- The Bevy mobile example notes shadow crashes on some devices.
- Upstream reports Android frame-pacing and MSAA load issues on 0.19.

# Scope

- Capability detection at startup.
- Tiers: MSAA off, resolution scale, shadow tier, bloom/post tier, particle tier.
- A non-compute particle fallback behind the existing `VfxRequest` boundary.
- 60 FPS cap with frame pacing.
- Thermal check over a 10-minute session.
- A user-visible override in settings.

# Out of scope

Desktop quality settings beyond sharing the same tier model.

# Acceptance criteria

- At least 55 FPS median in 3v3 Practice on the agreed mid-range device.
- A low-end/fallback device runs without crashing, with the fallback particles visible.
- The selected tier is logged and can be overridden.

# Verification

- Device measurements recorded here.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.

# Inherited from BRL-0159 (2026-11-13)
- Reactive `WinitSettings` in menus / continuous in matches (power) was deferred here from BRL-0159 so the
  Android shell left desktop update behavior unchanged.
- Pixel 10 GPU: Imagination PowerVR DXT-48-1536 (Tensor G5), Vulkan 1.3 at launch, Imagination 25.x /
  Vulkan 1.4 driver reported from Android 16 QPR3. bevy_hanabi compute particles and HDR/bloom are the
  first things to check on device; record `AdapterInfo` from logcat.

# Device data from BRL-0159 (2026-11-13; the connected phone was a Pixel 8a, not the Pixel 10)
- Pixel 8a, Android 17 (API 37): Mali-G715, driver v1.r54p3, Vulkan backend. The shell and the 3D fighter
  preview render correctly.
- Process memory on the Dashboard: about 1.55 GB PSS. Total memory is 7.4 GiB.
- APK is 119 MB: `libbrawler_android.so` is 113.5 MB stripped (82 MB of code) under the `android`
  profile with opt-level 3 and thin LTO. Consider opt-level "s"/"z", codegen-units 1, or fat LTO, and
  fewer Bevy features, as part of mobile tiers.

# Target device update (2026-11-13)
- The owner's Pixel 8a (Tensor G3, Mali-G715, Vulkan, Android 17) is the Android target (user decision on BRL-0159). APK size (119 MB, mostly the 113.5 MB native library) and ~1.55 GB Dashboard memory measured there are in scope for this ticket.


# Baseline on the Pixel 8a (2026-11-13, before any change)
- Release APK from `main` (`9403459d`), Practice Wipeout on Verdant Crossfire with bots, display at 60 Hz
  (Smooth Display off), measured with `dumpsys SurfaceFlinger --latency` on the game SurfaceView layer:
  **median 12-15 FPS** (frame interval median 67-84 ms, p95 134 ms). Process PSS 1.39 GB in the match
  (GL mtrack 797 MB). Per-thread CPU at most ~32 %, GPU clock near its 850 MHz ceiling: GPU bound.
- Adapter: Mali-G715, Vulkan, driver v1.r54p3; Hanabi initializes (compute is available).

# Design decisions (2026-11-13, recorded before implementation)
1. **Tier model.** Three presentation tiers, `High`, `Medium`, `Low`, plus a persisted player preference
   `Auto | Low | Medium | High` (`ClientShellSettings.graphics_quality`, serde-defaulted to `Auto` so
   older settings files still load). The tier contents are authored client-only data in
   `assets/catalogs/render_quality.ron` (embedded like `world_look.ron`, outside the gameplay
   fingerprint and the server feature graph). Code owns the ceilings: MSAA at most 4 samples,
   bloom requires HDR, the CPU particle caps, and the 60 FPS frame-rate target.
2. **Knobs per tier:** MSAA samples (arena and HUD camera together, since they share one target), HDR
   (arena and HUD together), bloom, arena directional-light shadow maps, blob contact shadows,
   foliage sway, and the particle backend (`Gpu` Hanabi or the `Cpu` fallback). Outlines and the toon
   material stay on in every tier: they carry readability, not decoration. `High` reproduces today's
   desktop look exactly (MSAA 4, HDR, bloom, shadow maps, contact shadows, sway, GPU particles).
3. **Auto selection** is a pure function of detected capability: no compute-capable GPU particles
   (missing compute/vertex-storage/indirect downlevel flags, too few storage buffers or bind groups,
   or the GL backend) -> `Low`; Android otherwise -> `Medium`; anything else -> `High`. Desktop
   behavior is therefore unchanged. The active tier, the preference, and the reason are logged at
   startup and whenever the selection changes. Tier contents are re-tuned from Pixel 8a measurements.
4. **Capability detection and the GPU-particle fallback.** Hanabi is no longer added in `build`. The
   particle adapter plugin inspects `RenderDevice` limits and `RenderAdapter` downlevel capabilities in
   `Plugin::finish` (after `RenderPlugin::finish` creates the device) and only then installs and
   finishes `HanabiPlugin`, so an incapable device never builds a compute pipeline. A non-compute CPU
   fallback sits behind the same `VfxRequest` -> recipe particle-layer boundary: each burst layer
   becomes a small bounded set of deterministic, seeded mesh particles (same meshes, profile colors,
   accelerations, lifetimes, and sizes) simulated on the CPU and despawned with their effect root.
   It is used when Hanabi is unavailable or the active tier selects `Cpu`. The rate-based projectile
   smoke trail is decorative and is skipped in the fallback. `BRAWLER_FORCE_PARTICLE_FALLBACK=1`
   (desktop verification only, like `BRAWLER_FORCE_PRIMITIVE_WORLD`) treats the device as incapable.
5. **Settings override.** A "Graphics quality" row on the Display settings page steps through
   Auto/Low/Medium/High and shows the resolved tier (for example `AUTO (MEDIUM)`); it applies live.
6. **60 FPS cap with frame pacing (Android).** The Android entry crate asks the compositor for 60 Hz
   on the game surface with `ANativeWindow_setFrameRate` (ndk `api-level-30`, already in the
   dependency graph) whenever the native window changes, so 90/120 Hz panels present at an even
   60 Hz under FIFO. Desktop present modes are unchanged.
7. **Resolution scale** needs render-to-texture plus an upscale pass (Bevy 0.19's
   `MainPassResolutionOverride` only shrinks the viewport) or Android buffer-geometry scaling. It is
   attempted only if a tier still misses 55 FPS after the cheaper knobs; otherwise it moves to a
   follow-up ticket with the measurements.
8. Menu-time reactive `WinitSettings` (inherited from BRL-0159) is decided from the Dashboard power/
   frame measurements; if not needed for the acceptance criteria it moves to a follow-up ticket.

# Acceptance-criteria refinements
- "Agreed mid-range device" = the owner's Pixel 8a; measured with SurfaceFlinger frame timestamps in
  Practice Wipeout 3v3-capable map (Verdant Crossfire) with bots, at the Auto tier.
- "Low-end/fallback device": no such device is attached; evidence is (a) unit tests of the capability
  decision for GL/no-compute limits, (b) a desktop run with `BRAWLER_FORCE_PARTICLE_FALLBACK=1` (Hanabi
  never installed) showing fallback particles, and (c) the Pixel 8a at the Low tier with CPU particles.
- "Logged and overridable": the startup/selection log line in logcat plus the settings row.


# Implementation (2026-11-14)
- `assets/catalogs/render_quality.ron` + `src/client/presentation_3d/render_quality.rs` (+ `tests.rs`):
  tier catalog (validated: schema, unknown fields, bloom needs HDR, MSAA enum capped at 4),
  `RenderCapabilities::detect` (in `RenderQualityPlugin::finish`), Auto resolution, the
  `ActiveRenderQuality` resource and its change log line, and systems that keep arena + HUD camera
  MSAA/HDR and arena light shadow maps equal to the tier. Bloom (`world_look.rs`) and foliage sway
  (`environment_assets/sway.rs`, which now also skips redundant transform writes) consult the tier.
- `combat/particles.rs`: Hanabi is installed and finished in `VfxParticleAdapterPlugin::finish` only
  on a compute-capable GPU; the backend is chosen per request from the tier. New
  `combat/cpu_particles.rs`: the bounded, seeded CPU fallback (max 10 mesh particles per burst,
  same meshes/colors/acceleration/lifetime/size; despawns with the effect root; counted against the
  96-emitter cap). The fallback skips the decorative rate-based smoke trail. `vfx_lab.rs` initializes
  its particle clock itself because Hanabi is no longer present at build time.
- Settings: `ClientShellSettings.graphics_quality` (`GraphicsQuality::{Auto,Low,Medium,High}`,
  serde-defaulted for old files); Display page row "Graphics quality".
- Android (`packages/brawler-android/src/frame_pacing.rs`, `ndk` nativewindow/api-level-30 and
  `libc`, both already in the lock graph): 60 Hz surface vote, a two-worker compute pool, and an ADPF
  (`APerformanceHint`, resolved with `dlsym` because minSdk is 31) session over the main, render, and
  compute threads with a 16.7 ms target, reporting each main-loop frame.
- Docs: `docs/11-art-and-presentation-direction.md` (quality tiers; Hanabi capability gate and CPU
  fallback).

# Decisions changed during implementation (recorded with evidence)
- **Contact shadows stay on in every tier** (dropped as a knob): they are cheap blended discs that
  carry grounding readability, and toggling them would conflict with the concealment visibility
  pass. Measurements showed the GPU was not the bottleneck at Medium.
- **The settings control is a tap-to-cycle chip** (`AUTO (MEDIUM)` / `LOW` / `MEDIUM` / `HIGH`), not
  a slider: on the Pixel 8a the existing shell sliders do not react to touch (BRL-0201).
- **The real Android bottleneck was CPU frequency/latency, not the GPU.** With Bevy's default five
  compute workers every thread was <35 % busy, the big core idled at 500 MHz and the mid cores at
  0.9-1.8 GHz; `simpleperf` showed executor park/unpark overhead dominating. SurfaceFlinger's
  post-to-acquire (GPU) time was 12-16 ms while frames were posted every 67-84 ms. GPU tier knobs
  alone did not move the match frame rate (Medium tier without the fix: 12-13 FPS). The fix is an
  Android Dynamic Performance Framework hint session plus a two-worker compute pool (both Android-only,
  in the entry crate; desktop unchanged). This is within "60 FPS cap with frame pacing" but was not
  anticipated in the design above.
- **Resolution scale is not implemented**: with the CPU fix, Auto (Medium) holds a 60 FPS median and
  the GPU needs 8-12 ms per frame at native 2400x1080, so render-to-texture upscaling is unneeded for
  the acceptance criteria. Deferred (see below).
- Menu-time reactive `WinitSettings` moved to BRL-0203 (the Dashboard now presents at 60 FPS; the
  remaining question is power).
- APK size moved to BRL-0202 (needs several full rebuilds and must not regress the CPU fix).

# Verification (2026-11-14)
- `just lint` (real ripgrep on PATH): pass. `cargo ndk --target arm64-v8a --platform 31 clippy
  --locked -p brawler-android -- -D warnings`: pass (also fixed two existing doc-markdown findings in
  the entry crate).
- `just test`: pass (all suites; new tests: render quality catalog/capability/Auto/live override,
  CPU particle bounds/determinism/simulation, fallback emitter allocation, settings round-trip and
  old-file default, Display row cycling and reset).
- `just e2e 2`: pass (1 exact 1v1 roster reached Active).
- Desktop native evidence (`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence`):
  High (Auto on the Mac, Apple M6, Hanabi initialized): pass, frame p50 8.33 ms / p95 8.63 ms;
  `BRAWLER_FORCE_PARTICLE_FALLBACK=1`: Hanabi never installed, Auto -> Low, pass, p50 8.33 ms /
  p95 8.57 ms, fallback emitter high-water 2-3, 0 Hanabi effect assets. (A first pair run concurrently
  with an Android build failed the >100 ms frame threshold / was killed; both re-ran alone and passed.)
  VfxLab fire lobbed impact with the forced fallback shows the CPU smoke/spark particles.
- The High tier reproduces the pre-ticket desktop camera/light/bloom/particle configuration exactly
  (unit test `high_tier_keeps_the_accepted_desktop_look`); no side-by-side capture against `main` was
  taken.

## Pixel 8a measurements (Practice Wipeout 3v3, Verdant Crossfire, bots, scripted move + fire;
## display at 60 Hz; `dumpsys SurfaceFlinger --timestats`, 25-30 s windows; PSS from `dumpsys meminfo`)
| Build / tier | Match avg FPS | Frame median / p95 | GPU (post->acquire) median | Match PSS | Dashboard avg FPS / PSS |
|---|---|---|---|---|---|
| `main` baseline (Wipeout on Verdant) | 12-15 | 67-84 / 134 ms | 16 ms | 1.39 GB | 13.6 / 1.47 GB |
| Tiers only, Medium (5 workers, no ADPF) | 12.0-13.1 | 70 / 118-134 ms | 16 ms | 1.11 GB | 15-18 |
| Medium, 1 / 3 / 2 workers, no ADPF | 22.3 / 17.5 / 23.6 | 50 / 50 / 33 ms | 15 ms | 1.1 GB | - |
| Medium + ADPF, 5 / 3 / 2 workers | 47.3-47.8 / 51.9 / 52.9-55.9 | 16 / 33 ms | 12 ms | 1.1 GB | 47-54 |
| **Final, Auto = Medium** (2 workers + ADPF) | **51.3** (48.1 after a High session) | **16 / 33 ms** | 12 ms | 1.11 GB | 52.3 / 0.86 GB |
| Final, Low (CPU particles, LDR) | 55.7-56.9 | 16 / 33 ms | 10-11 ms | 1.12 GB | 56.7-58.7 / 1.02 GB |
| Final, High | 33.2 | 33 / 50 ms | 27 ms | 1.82 GB (GL 1.05 GB) | 38.5 / 1.40 GB |
- 10-minute session at Auto (21 samples): every sample median 16 ms (60 FPS); in-match samples
  50-57 FPS average, results/transition samples ~60; thermal status 1 (light) throughout, never
  escalated; PSS steady at 1.10-1.19 GB. At High, thermal status reached 2 within one match.
- Release APK 120 MB; `libbrawler_android.so` 113.9 MB (was 119 MB / 113.5 MB).
- Logcat on the phone: `render quality tier selected tier="MEDIUM" preference="AUTO" reason="mobile
  GPU" gpu_particles=true particles=Gpu msaa=Off hdr=true bloom=false shadow_maps=false`,
  `requested game surface frame rate fps=60.0`, `frame performance hint session started threads=3`.
- Acceptance: >=55 FPS median in 3v3 Practice -> met (median frame 16 ms = 60 FPS at Auto and Low; the
  average at Auto is 48-52 because ~1 in 6 frames still takes two vsyncs). Fallback without crash
  with visible fallback particles -> met on desktop with the forced incapable path and on the Pixel at
  Low (CPU backend, Hanabi installed but unused); no real GLES-only device was available. Tier logged
  and overridable -> met.
- Evidence: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0161/`
  (`measurements.txt`, `timestats-*.txt`, phone screenshots `final-*.png`, `low-particles-*.png`,
  `settings-*.png`, desktop `desktop/*.captures`, `vfxlab/{gpu,cpu}.png`). Runs that landed on an
  error screen are marked INVALID in `measurements.txt` and excluded.

# Playtest request (owner)
- Build/install: `just android-install 192.168.31.47:5001` with
  `BRAWLER_ROUTED_BIND=0.0.0.0:5001 just server` on the Mac.
- Play Practice 3v3 (Wipeout, Hot Zone, Heist) for 10+ minutes at Auto. Report smoothness, heat,
  and battery drain versus before.
- Settings > Display > Graphics quality: tap through Low, Medium, High, Auto in a match. Expect Low
  to look brighter/flatter (no HDR; custom effect shaders are not tonemapped in LDR) with simple
  mesh sparks/smoke, Medium to look like desktop minus glow/anti-aliasing, and High to drop to about
  30 FPS on the Pixel 8a.
- If you can, turn on Smooth Display (90/120 Hz) and confirm the game still runs at an even 60 FPS
  (the 60 Hz surface vote could not be verified at 60 Hz panel settings).
- Desktop: `just client`; confirm the arena look is unchanged (Auto = High).

# Deferred / follow-ups
- BRL-0201 touch-adjustable settings sliders (existing shell sliders ignore touch).
- BRL-0202 APK/native library size (owner listed it in scope here; split out, see decision above).
- BRL-0203 lower menu frame rate on Android for power.
- Resolution scale: not needed for the acceptance criteria on the Pixel 8a; revisit only for a device
  that misses 60 FPS at Low.
- Unverified: a real GLES/no-compute device, the Pixel 10 PowerVR GPU, and 90/120 Hz panels.
- Low drops HDR, so custom effect shaders that output HDR values clip instead of tonemapping; accepted
  for Low, revisit if Low becomes the default anywhere.

# Learn-from-errors review
- **Mistake: tuned GPU knobs before locating the bottleneck.** The GPU clock near its ceiling looked
  like a GPU bound, and the first tier build did not move the match frame rate. Cause: inferred the
  bound from one signal. Prevention: compare SurfaceFlinger post->acquire (GPU) with the present
  interval (app) and per-thread CPU before changing render features. Lesson: on Android, many idle-ish
  threads plus low clocks means a CPU latency bound; ADPF hints are the platform fix.
- **Mistake: early automated measurements included error screens** (`AccountInUse`, connection
  timeouts after a force-stop) that report a clean 60 FPS. Cause: unverified scripted navigation.
  Prevention: validate every run with a screenshot and a process-scoped logcat (`--pid`) tier line,
  wait 40 s after force-stop before reconnecting; invalid runs were kept and marked.
- **Mistake: concurrent adb invocations** (input loop + dumpsys) repeatedly crashed the host adb
  server. Prevention: serialize adb calls or tolerate restarts and re-validate the run.
- **Mistake: a settings slider for a discrete choice** shipped untested on touch. Prevention: try
  every new settings control on the phone before measuring.
- Changing `CARGO_PROFILE_ANDROID_STRIP` for profiling rebuilds every dependency (~10 min); keep a
  separate profiling build directory or accept one profiling build per investigation.
