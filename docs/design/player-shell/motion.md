# Player shell motion

The shell is energetic, matching the game's toy-like direction ([docs/11](../../11-art-and-presentation-direction.md#visual-direction), BRL-0140). Navigation and accepted state changes still occur immediately. Motion decorates them; it never owns saving, admission, or route completion, and it never delays input.

## Interaction feedback

| State | Behavior |
|---|---|
| Hover / focus | Immediate brightness lift and the `accent` focus frame. Hit areas stay fixed. |
| Press | Squash to about 0.95 scale over about 60 ms. The label drops about 3 px and the bottom lip collapses. Feedback is visible on press, but the action activates on release. |
| Release | A short overshoot back to rest (about 120 ms, ease-out-back), then the action fires. A drag past the cancel threshold returns to rest with no action. |
| Disabled | Muted fill, no squash, and factual wording. |

## Idle and ambient motion

- **Primary action (Play):** gentle breathing scale (about ±2%, 1.6 s period), plus a periodic diagonal shine sweep (about every 4 s).
- **Background:** slow drift of the Dashboard background's tonal planes. Sparse, low-contrast toy motifs (rings, plus signs, diamonds, sparkles) drift with the planes and bob gently; they stay clear of the logo, hero, and controls. No noise or random flicker. The Low render tier drops the motifs.
- **Brawler preview:** idle animation from the character's own clip, with an occasional flourish when the character is selected.

## Transitions

- **Screen and panel entrances:** about 150–250 ms, a short slide or scale-in from about 0.92 with ease-out-back. Interruptible, so a new navigation cancels the running transition. Entrances play when the visible flow screen or overlay changes, not when an unchanged screen rebuilds (for example after a Settings toggle or while typing).
- **Rewards, confirmations, and match results:** one short burst (about 420 ms: a 7% scale pop with an ease-out-back settle, one shine sweep, and a brief face flash toward white) on the element that changed. Bursts are screen-space UI and never brighter than white, so they cannot bloom. A navigation that leaves the screen immediately (leaving a match, changing server) shows its destination's entrance instead; the burst never delays it.
- **Loading:** an indeterminate activity animation while real progress is pending. Never infer progress percentages from time.

## Reduced motion and reduced effects

Idle breathing, shine, background drift (planes and motifs freeze in place), entrance slides, and reward bursts are disabled. Press and focus feedback stay immediate, as a color and lip change without scale. Loading still conveys pending status in text.

## Ownership

`shell_surface.wgsl` and the shared surface/control components own interaction feedback, so screens do not animate their own controls. Screens decide which element bursts; the shared surface code owns how it looks. Preview flourishes belong to the owning screen ticket and need native review. BRL-0156 delivers the shared interaction, idle, and entrance motion; BRL-51zd adds bursts and motifs.

## Implementation (BRL-0156)

| Behavior | Native owner |
|---|---|
| Release activation and drag-cancel (12 logical px) | `ReleaseActivation` in `packages/brawler-client/src/ui/press.rs`, required by flow, shell, match-menu, Practice map and gallery controls |
| Press squash/drop, release overshoot, entrance, Play breathing and shine | `SurfaceMotion` in `packages/brawler-client/src/ui/surface.rs`; scale and drop use `UiTransform`, so layout and hit areas never move |
| Lip collapse and shine | `shell_surface.wgsl` (`motion` uniform) |
| Which surface idles | The owning screen adds `PrimaryIdleMotion`; only Dashboard Play does today |
| Entrance trigger | The flow records destination changes in `SurfaceEntrance` |
| Reduced motion | `SurfaceMotionPolicy`, synced from Reduced motion or Reduced combat effects |
| Reward/confirmation burst | `SurfaceBurst` in `surface.rs` (pop, shine and flash on surfaces; pop only on plain nodes such as the results headline); flash is the `shell_surface.wgsl` `motion.z` uniform |
| Which element bursts | Results: the trophy, outcome headline and local-team card on the first presentation of a result. Flow commits with `celebrate`: the focused action after an accepted weapon-part change (the slot), an accepted ability change (the ability slot), and Delete on the brawler confirmation while the server decides. `FlowBurst` keeps the request open for 250 ms so the rebuilt screen's action receives it |
| Background motifs | `ShellBackdrop` in `packages/brawler-client/src/ui/motifs.rs` adds a `shell_motifs.wgsl` layer to the Dashboard background (Dashboard layout) and to every `game-selection-background` screen (edges layout). Positions, shapes and opacity live in the shader |
| Motif tier | `shell_motifs` in `assets/catalogs/render_quality.ron` (High/Medium on, Low off), synced into `SurfaceMotionPolicy.motifs` |

Motion state is presentation only. Input reads `ReleaseActivation`, never `SurfaceMotion` or `SurfaceBurst`, and an action is never delayed until an animation completes. Preview flourishes remain owned by a later screen ticket.
