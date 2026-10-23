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
- **Background:** slow drift of the sparse motif layer. No noise or random flicker.
- **Brawler preview:** idle animation from the character's own clip, with an occasional flourish when the character is selected.

## Transitions

- **Screen and panel entrances:** about 150–250 ms, a short slide or scale-in from about 0.92 with ease-out-back. Interruptible, so a new navigation cancels the running transition.
- **Rewards, confirmations, and match results:** one short burst (scale pop plus particles or shine) on the element that changed.
- **Loading:** an indeterminate activity animation while real progress is pending. Never infer progress percentages from time.

## Reduced motion and reduced effects

Idle breathing, shine, background drift, entrance slides, and reward bursts are disabled. Press and focus feedback stay immediate, as a color and lip change without scale. Loading still conveys pending status in text.

## Ownership

`shell_surface.wgsl` and the shared surface/control components own interaction feedback, so screens do not animate their own controls. Screen-specific choreography (results bursts, preview flourishes) belongs to the owning screen ticket and needs native review. BRL-0156 delivers the shared interaction, idle, and entrance motion.
