Two fullscreen preview mockups: live competitive scoreboard with two team groups, team score and player status; results with outcome, final team scores, mode and replay/dashboard actions. Use cream/teal/orange/navy #081932, clean geometric background, slanted cut-corner shared actions. No invented kills, assists, damage totals, rewards, ranks or XP. Current results context retains outcome/local team/game name/final mode score but no final player roster, so results must not claim a retained per-player report. Scoreboard shows roster/status rather than raw internal W1/HP?/B0/debug strings. Illustrative names and scores only. Review mode-specific adaptations (Hot Zone/Heist/Zombie) before implementation. Preserve held scoreboard versus menu-latched return semantics in eventual implementation. Design only until user acceptance.


Generated scoreboard-v1 and results-v1 proposals with built-in image_gen and saved PNG/notes under asset_src/player-shell/concepts. Reviewed text and hierarchy. Names and scores are illustrative. Results generator added an inset line to Dashboard too; implementation must use the shared single focused action (Play Again) rather than treat both outlines as focus. Scoreboard status must respect observer-authorized data; unknown is not Alive. Back is for menu-latched scoreboard, held scoreboard returns on release. Awaiting user design feedback; no runtime changes.


User correction supersedes the v1 restriction to existing presentation data: Wipeout needs per-player Kills and Deaths, grouped by team with team scores prominent, in both live scoreboard and final results. Replace Alive/Respawning columns. Runtime work must add authoritative attribution and retained final player statistics if absent; do not derive counters from client-visible effects. Revise mockups first. Illustrative live totals: own team kills 6/deaths 4, opponents kills 4/deaths 6. Final: 10/6 versus 6/10. No XP or unrelated statistics.


V2 generated with built-in image_gen and saved as scoreboard-v2.png/results-v2.png with wipeout-stats-v2.md. Both show Kills/Deaths; illustrative sums reconciled with team scores. Removed duplicated final totals and extra Dashboard focus inset after visual inspection. Results now needs retained final per-player statistics, explicitly superseding v1. No runtime changes in this design revision; implementation must validate server attribution, environmental deaths and disconnected participant retention. Await user visual feedback.


## Acceptance
User accepted the corrected results-v2 with “last one is good go for it”. Implementation of authoritative statistics and both native views continues in BRL-0133. The earlier restriction to existing team-only data is superseded by the requested kills/deaths breakdown.
