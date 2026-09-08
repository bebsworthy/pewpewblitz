# Player shell motion

Navigation and accepted state changes occur immediately; presentation never owns saving, admission or route completion.

- Resting background: slow bounded drift of the two geometric plane edges (about 0.4% of screen height). No noise, particles or random patterns.
- Hover/focus: clear contrast change immediately. Preserve a distinct focus outline and fixed hit areas. Press has a darker fill; disabled actions have muted fill and factual wording.
- Later screen transitions: brief opacity/position treatment, around 150–200 ms, interruptible. Implement only when the owning screen slice needs it.
- Loading: indeterminate activity animation while real progress is pending; never infer progress percentages from time.
- Reduced motion or reduced effects: background edges fixed, no camera sway or automatic flourish; immediate focus/pressed feedback remains. Loading still conveys pending status in text.

The current Dashboard slice implements bounded background motion and interaction colors. The real model preview retains its existing lifecycle. Further transition choreography needs native review and its owning ticket.
