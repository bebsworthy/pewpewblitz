# Fighter picker v1 — proposed

BRL-0124. Generated with the built-in image tool. Pending user review.

## Exact prompt

Use case: ui-mockup. Generate a fullscreen 16:9 CHOOSE FIGHTER screen for PewPew Blitz brawler creation. Reference image 1 is the approved passive picker: match its shared header, card styling, focus treatment, clean cream navy teal orange palette. Reference image 2 provides illustrative robot visual direction only.
Header: one BACK control upper left, CHOOSE FIGHTER centered, CREATE BRAWLER small upper right. Under title a single restrained sentence: "Fighter is fixed after creation."
Main content: three medium-sized equal portrait cards in a centered single row, each around 26% screen width, all fully visible with comfortable margins. No separate preview panel. Each whole card is clickable. Cards feature polished friendly robot concept renders, then bold name and concise tradeoff text, never a spreadsheet.
DEFAULT: balanced orange/white robot, "Balanced health and speed." Teal-filled card denotes current draft choice.
LIGHTWEIGHT: slimmer teal/white robot with the same family design, "Faster movement. Less health." Cream card, orange focus outline with one orange left-edge chevron. This is the sole focused card.
REINFORCED: broader orange/navy armored robot in the same family, "More health. Slower movement." Cream card navy outline.
Keep renders contained inside cards, exact consistent typography and alignments. No SELECTED labels, locks, progression, currency, fake stats, save or confirm buttons, controller instructions, or second back. Background flat warm cream with restrained teal diagonal lower-left and orange lower-edge sweep matching references. No grime, patterns, architecture, logo, or unnecessary decoration. Attractive clean game UI, not administrative software.

## Implementation notes

Three actual advertised profiles: Default, Lightweight, Reinforced. Full-card selection updates the creation draft and returns to Create Brawler; no persistence until Create. Back preserves the draft. Teal is current choice, orange outline and chevron are focus. Fighter is immutable after creation. Generated robot models are illustrative stand-ins, not replacement asset approvals. Use actual project models and bundled font. This three-item catalog uses three cards; if catalog grows, cap card size and use a responsive scrolling grid rather than adding a separate preview. No runtime code changed.
