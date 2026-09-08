# Full-screen Practice map picker — revision 02

Approved by the user on 2026-09-08. Replaces revision 01 popup and text-only cards.

Full-screen composition with actual recipe-derived top-down overviews, map names, current mode/team size and one Start Practice action. Orange indicates choice; frame/pointer indicates navigation focus. No Selected text/checkmarks. Escape/gamepad East cancels without starting a session.

Overviews are schematic, not gameplay screenshots: actual map dimensions, placement coordinates, filled rectangles, rotated catalog footprints and team spawns; simplified semantic colors. Map aspect ratios are preserved. Render script checks complete parsing of the two current authored recipes. It does not replace the production Rust map resolver or render dynamic match state.

Rebuild: run render.py with Python fontTools installed, then rasterize practice-map-picker.svg with Sharp. Typography uses glyph outlines from the shipped Lilita One font, keeping this standalone design artifact portable. Production text remains native text. No imaging model was used for this revision, so geometry remains tied to source data.

Files: practice-map-picker.png and .svg; independent feature-yard-wipeout-overview.svg and verdant-crossfire-overview.svg; render.py is the reproducible source.
