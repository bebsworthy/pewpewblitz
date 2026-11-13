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
