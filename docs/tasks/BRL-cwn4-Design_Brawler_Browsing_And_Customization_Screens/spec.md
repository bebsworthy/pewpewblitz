## Scope and sequence
User accepted tackling brawler screens next, starting with mockups. Weapon customization is already approved; broader brawler flows are not. First produce a brawler browser concept for review, then details, creation and ability selection using the accepted direction. Do not change runtime UI before this design review.

## Existing behavior to preserve
Saved-brawler cards open details. Details provides selection for play, ability customization, weapon customization and deletion. Creation chooses immutable fighter and weapon bases plus a starting ultimate. Existing ability editor supports name, ultimate and two passives. Saved brawlers are player-authored builds, not unlockable heroes or purchased characters.

## Design constraints
Match approved clean Dashboard reference (exec-bc166722-8f01-4b02-8c36-9d6a01ffc483.png): cream/navy/teal/orange, bold bundled-font direction, slanted framed panels, restrained shadows, simple diagonal background fields. No dirt, arbitrary patterns, architectural scenery, new logo, currencies, rarity, levels or invented unlocks. Reuse production toolkit interaction states. Navigation focus and chosen-for-play state are distinct; avoid redundant Selected labels and input-device footers. One Back. Prefer fullscreen pages and visual choices; avoid oversized spreadsheet rows. Character illustrations in generated concepts are composition stand-ins, not approved model replacements.

## Browser proposal
Saved build grid with compact illustrated cards and a Create brawler tile; space for a larger preview of the focused build. Whole card opens details. Teal communicates current play selection; orange outline/offset communicates focus. Scroll the grid vertically when needed, leaving header and preview stable. Empty state features the Create tile. Use example build names only as mockup content.

## Deliverables and acceptance
Generate a first browser mockup using the built-in image tool and approved reference, inspect readability/layout/style, present for user feedback. Record accepted/corrected direction before preparing remaining screens. Keep ticket doing while design approval remains outstanding. No implementation or new gameplay behavior in this ticket.


First browser proposal generated using built-in image_gen and visually inspected. Saved concept: asset_src/player-shell/concepts/brawlers-browser-v1.png; generation brief and interaction notes beside it in brawlers-browser-v1.md. Three-by-two grid (five saved build examples plus Create), large focused-build preview, clean Dashboard palette and angular chrome, distinct selected/focus states. All cards and labels fit. Robot renders and ability pictograms are illustrative stand-ins. User review pending; details/creation/abilities will follow the accepted browser direction. No runtime changes.


User feedback: browser direction liked overall, but remove the large right-side preview because the collection may contain dozens of characters. Revise to a full-width vertically scrolling grid, five compact cards per desktop row with responsive columns. Keep one Create tile in the grid and a fixed header. Preserve selected/focus distinction, whole-card entry into details, approved clean style, and no redundant instructions or Selected label. Generate a revised mockup before implementing.


Revised browser v2 generated and inspected: asset_src/player-shell/concepts/brawlers-browser-v2.png, with exact generation prompt and review notes in adjacent markdown. Large preview removed; full-width five-column scrolling grid, two complete rows plus a partial third, Create first. Implementation must use only one orange focus outline; generated Spark's extra orange border is an artifact, not a second focus state. Selected state remains teal. Awaiting review; no runtime edits.


User approved browser v2 ("Good !"). Accepted direction: full-width responsive grid, five columns at desktop size, vertical scrolling, fixed header, Create brawler first, no separate large preview. Preserve one orange navigation-focus outline and teal selected-for-play state; the extra orange border on the generated selected card is not part of the interaction specification. Browser design approval is complete. Next design deliverable is brawler details, followed by creation and ability customization; these remain unapproved and no runtime implementation is authorized by this approval alone.


Proceeding with brawler details mockup after user "go". Detail page: one Back to Brawlers, saved build name with an understated rename affordance, large current-model preview, whole clickable Weapon and Abilities panels, distinct Select for play action, low-priority Delete action. Keep immutable base information concise, no duplicate selected text or Save button, no currencies/progression, no new gameplay controls. Example content is illustrative. Preserve actual details capabilities; selection returns to Dashboard after server acceptance. This is design work, not runtime implementation.


Brawler details v1 generated and inspected: asset_src/player-shell/concepts/brawler-details-v1.png, exact prompt and interaction notes in adjacent markdown. Character left; clickable Weapon and Abilities panels right; one Select for play action; rename by title; low-priority Delete. No nested customization buttons or Save. Example Arc build uses Arc Launcher, Dash, Lightweight Frame and Tenacity. Models/icons remain stand-ins. Immutable fighter-base facts are not represented yet and need minimal useful presentation review; no invented numeric stats. Awaiting user feedback.


User approved brawler details v1 ("ok"). Accepted layout: character preview left, whole clickable Weapon and Abilities panels right, title rename affordance, one Select for play action, separate low-priority Delete. Weapon attachment slots are summaries, not nested actions. Preserve actual data and existing models; generated art remains illustrative. Browser and details designs are now approved. Creation and ability customization remain pending; Create Brawler is next. No runtime changes in this approval closeout.


Create Brawler v1 generated and inspected: asset_src/player-shell/concepts/brawler-create-v1.png; exact prompt and interaction notes in adjacent markdown. Three illustrated cards (fighter Default, weapon Pulse, ultimate Dash) open fullscreen pickers rather than cycle text. Name field plus one Create action. Fighter/weapon immutability stated once as meaningful decision information. Name-at-creation is a proposed UX change; check existing atomic request support before implementation and do not silently create-then-rename. All choices must use server-advertised catalog data. No runtime changes; user review pending.


User approved Create Brawler v1 ("ok"). Accepted design: three illustrated fighter/weapon/ultimate choices opening fullscreen pickers, editable name before one Create action, and a single concise explanation that fighter and weapon are fixed after creation. Preserve server-advertised choices and validate atomic name-at-creation support during implementation. Browser, details and creation overview designs are approved. Ability customization and choice-picker designs remain outstanding. No runtime changes.


User "goi" interpreted as go for ability customization mockup. Proposal: one ultimate and two passive cards, fullscreen pickers, autosave valid changes, no Save button. Existing editor currently uses explicit save; autosave is proposed consistent with the user's earlier preference and must preserve server acceptance/error handling in implementation. Name editing remains on details; this page owns ability slots only.


Ability customization v1 generated and inspected: asset_src/player-shell/concepts/brawler-abilities-v1.png; exact prompt and behavior notes in adjacent markdown. One ultimate and two passive cards with names and short effect descriptions, single Back, build-name context, no Save or duplicated equipped labels. Cards open fullscreen grids; accepted valid edits autosave through authoritative profile updates. Icons and short copy remain illustrative until mapped to actual catalog semantics. User review pending; no runtime changes.


User approved ability customization v1 and requested continuation ("good continue"). Next: fullscreen choice pickers. Start with ultimate picker using five-column responsive, vertically scrolling visual grid; current choice teal and focus orange; one Back returns unchanged, activating a valid card applies and returns to the parent. Creation context updates a local creation draft; existing-build context persists via authoritative profile update. No extra Select/Save button or Nothing option for required ultimate. Reuse grid pattern for passive, fighter and weapon pickers with each domain's actual eligibility and descriptions.


Ultimate picker v1 generated and inspected: asset_src/player-shell/concepts/ultimate-picker-v1.png, exact prompt and behavior notes beside it. Five-column vertically scrolling grid, ten full cards plus eleventh partial, clear teal current choice versus orange focus, concise effect summaries. One Back; card activation returns to parent with chosen value. Icons/copy remain subject to production catalog mapping. User review pending; no runtime edits.


User approved ultimate picker v1 and requested continuation ("great continue"). Next deliverable: passive picker using the same compact fullscreen scrolling grid, slot context in header, real authored passive names and concise tradeoff summaries. Preserve server rules; no invented locks or progression. Current slot value uses teal; focus uses orange. Do not duplicate Selected labels or add a Save button.


Passive picker v1 generated and saved at asset_src/player-shell/concepts/passive-picker-v1.png with exact prompt and behavior notes in adjacent markdown. Nine actual catalog passives in five columns, teal current value, orange focus, muted Tenacity with In passive 2 explanation. Duplicate/frame-family validation verified in src/builds/tests.rs; no unsupported Nothing choice. Quick Cycle and Adrenal Response trigger copy checked against src/abilities/passives.rs; final Quick Cycle wording should clarify defeating an enemy. Mockup pending user review. No runtime code changes.


User approved passive picker v1. Continue with fighter-base picker for creation: actual Default, Lightweight and Reinforced profiles, full-card choice updating creation draft and returning. Use shared picker styling, no separate large preview, no fake roster entries. Base remains immutable after creation.


Fighter picker v1 saved at asset_src/player-shell/concepts/fighter-picker-v1.png with exact generation prompt and implementation notes alongside. Three actual advertised profiles, current/focus distinction, whole-card draft selection and return, one permanence sentence. Generated models are illustrative only. Visual inspected: labels fit, one focus, no separate preview. Pending user review. Weapon-base picker remains next.


User approved fighter picker v1 and requested continuation. Next: weapon-base picker for creation, using actual advertised weapon presets, whole-card draft choice and return, current/focus distinction, no separate preview or Save button.


Weapon-base picker v1 saved at asset_src/player-shell/concepts/weapon-base-picker-v1.png with exact prompt and behavior notes alongside. Seven actual presets and effects verified against content/catalogs/weapons.ron. Five-column grid, current/focus distinction, creation-draft selection returns without separate Save. Visual inspected; Arc Launcher text needs more bottom padding in implementation. Models illustrative. Pending user review; after acceptance reconcile design handoff and implementation scope. No runtime changes.


User approved weapon-base picker v1 and requested continuation. All proposed browser/details/creation/abilities and four choice pickers approved. Design closeout: exact prompts and review notes saved beside PNGs; generated models are stand-ins. Lessons: remove duplicated state/instructions, distinguish selection from focus, size galleries for catalog growth, and keep actual gameplay semantics in copy. Runtime implementation follows in separate owned tickets; design acceptance complete.
