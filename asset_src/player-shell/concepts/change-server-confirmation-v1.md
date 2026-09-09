# CHANGE SERVER? — confirmation proposal

BRL-0129. Built-in image_gen edit of approved delete-confirmation-v1.png (option 2). Preserve cream/navy/teal/orange background, type family and slanted buttons with top-right/bottom-left cuts. Replace heading with CHANGE SERVER?, remove example brawler name, use semantic exit/switch icon. Consequence: Disconnect from the current lobby. Actions: Stay Connected / Change Server. Left action initially focused; Escape invokes it. No redundant Back/X, controller hints or modal backdrop.

Native implementation must reuse shared Surface geometry and current authority/connection paths, including pending and rejection handling. Gameplay remains live during Leave Match confirmation; no implicit pause. Compact actions stack with bounded scrolling. No penalties or data-loss claims added.

Inspected generated result: full labels fit, correct button silhouettes and focus treatment, readable consequence, no extraneous controls. No runtime changes. Variant awaits user review.
