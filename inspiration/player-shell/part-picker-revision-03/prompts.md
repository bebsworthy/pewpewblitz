# Full-screen part picker — BRL-0113

Built-in imagegen. Three-column tile grid, Nothing first, one Back, no separate Unequip/Save. Visually inspected requested elements; generated tiles are near-square rather than exact geometric squares, to be normalized in native layout. Auto-save selection semantics remain server-authoritative. Sample inventory and part renders are illustrative. Loading revision 02 approved separately by user. Picker awaits feedback.

## Exact prompt

Redesign supplied part-picker popup into a FULL-SCREEN game equipment chooser. Keep approved clean cream/teal/orange/navy palette, matte toy part thumbnails, crisp dark outlines and shallow tactile depth, minimal texture. No dimmed overview behind, no modal outer container or X close. Entire viewport is opaque cream with a restrained teal/orange corner color plane; no random patterns.
Header one BACK TO WEAPON control top-left, centered CHOOSE PART, small SLOT 1 / SPARK at right. Main content is a THREE COLUMN GRID of equal SQUARE SELECTABLE TILES with generous spacing. At 16:9, show first row fully and second row partly if needed to keep tiles square and text legible; slim scrollbar on far right, header stable. Each tile is one clickable item, no nested buttons. Prioritize clear consistent square card dimensions over cramming everything into screen. Use about 330px square cards at 1672px-wide reference, centered grid with 30px gutters; two rows can fit under modest header at approx 100px.
Reading order left-to-right:
1. NOTHING — first tile upper-left. Empty outlined socket/simple minus symbol, label 'Nothing'. This is a normal selectable item that clears the slot, same dimensions/style as all other tiles. NOT disabled, NOT destructive red, no Unequip button anywhere.
2. EXPANDED FEED — clean toy part thumbnail, 'More capacity', 'Slower refill', clear check plus Equipped badge because currently selected for Slot 1.
3. QUICK LOADER — clean part thumbnail, 'Less capacity', 'Faster refill'.
4. HEAVY PAYLOAD — thumbnail, 'More damage', 'Slower firing', small 'Slot 2' marker.
5. LONG ASSEMBLY — thumbnail, 'More reach', 'Slower firing'.
6. COMPACT ASSEMBLY — thumbnail, 'Faster firing', 'Less reach'.
Each part card: thumbnail upper half, readable title then two concise tradeoff lines lower half, check/status marker in corner where relevant. Text stays inside padding. No rarity, price, inventory count, unknown stat values or fabricated progression. Positive/negative words readable independent of teal/orange accents.
Auto-save on selecting Nothing or valid part; no Save/Apply/Confirm, no persistent instruction or technical note, no footer toolbar, no separate Unequip. Single Back only. Elegant game equipment collection rather than spreadsheet rows. One full-frame landscape screen.
