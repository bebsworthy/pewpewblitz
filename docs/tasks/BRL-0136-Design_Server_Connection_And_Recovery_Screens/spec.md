Design an approved visual family for server selection, connection progress, and connection failure/recovery before implementation.

Use the established PewPew Blitz player-shell language: clean cream field with restrained teal/orange geometry, navy ink rather than black, bold bundled typography, cut top-right/bottom-left corners, short offset shadows, and clear gamepad focus. Avoid dirty texture, random patterns, modal popups, duplicated labels, controller-specific button hints, and irrelevant settings/quit actions inside transient connection states.

Server selection: full-screen page with title, player name and server address fields, one primary CONNECT action, compact favorite/recent server cards, and settings/quit as understated footer actions. Connecting: full-screen status page with logo, clear phase and destination, one restrained progress indicator and CANCEL only. Recovery: full-screen page replacing the failed state rather than overlaying it; concise factual error, RETRY primary action, CHANGE SERVER secondary action. Exact copy should remain short and actionable. Produce native-screen proportion mockups and preserve project copies under asset_src/player-shell/concepts. User visual acceptance is required before implementation.


Generated three candidate screens with the built-in image tool and preserved them as server-select-v1.png, connecting-v1.png, and connection-lost-v1.png with adjacent provenance/implementation notes. Results-v2 provided the established visual-language reference; server-select-v1 then anchored the transient states. Design decisions: clean full-screen recovery rather than modal overlay; one action on Connecting; no Settings/ Quit inside Connecting; concise factual error; selected color without Selected text; common cut-corner button and visible focus frame. Await user visual feedback before implementation.


## Final user direction

The general layout is approved for implementation. Use the existing real logo asset in place of the generated approximation, remove the chain-link icon from connection recovery, and build the screens from shared UI components, adding focused reusable primitives only where they do not yet exist. No additional design screenshots are required.
