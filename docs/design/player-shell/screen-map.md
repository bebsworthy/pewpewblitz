# Player shell screen map

Current behavior/design contract. Approved composition details live in [README](README.md). Existing source and [Player UX](../../13-player-ux.md) own server authority and lifecycle.

| Surface | Purpose and essential controls | Destination / behavior |
|---|---|---|
| Connecting | Real preparation/connection phase, cancellation, recovery/settings | Accepted lobby → Dashboard; failure → recovery/server select |
| Server Select | Address and player identity, real favorites, Connect, Settings | Routed connection; no local/offline gameplay substitute |
| Dashboard (approved) | Hero/build identity, whole mode card, Play, Practice, Menu | Brawlers, game selection, authoritative admission, Menu |
| Brawlers | Saved profiles, persistent selection distinct from focus, Create | Detail or server-accepted selected profile |
| Brawler Detail | Actual build and hero, select, customize, delete, Back | Dashboard after accepted selection; editors or list |
| Create Brawler | Catalog-backed permanent base choices, create pending/failure | Accepted profile → details |
| Customize Abilities | Existing name/ability draft and validation controls | Preserve its existing Save/Back contract |
| Customize Weapon (approved) | Hero weapon, four numbered slot tiles, borderless factual stats | Slot → fullscreen parts picker; one Back to brawler |
| Parts Picker (approved) | Three columns, Nothing first, thumbnails, benefit/tradeoff, equipped state | Choice auto-saves through server; accepted → overview; failure stays; Back changes nothing |
| Game Type Select | Advertised types and factual rules/team sizes, persistent choice/focus | Selection/Back → Dashboard; no admission on card activation |
| Queue | Actual queue/formation facts, cancel | Reservation → loading; acknowledged cancel → Dashboard |
| Match Loading (approved) | Abstract background, hero, mode badge/title/team size, live phase/ring, cancel | Match start or authoritative cancellation/recovery |
| Match Complete bridge | Real result preserved while routing returns | Results when lobby ready |
| Results | Actual outcome/statistics, Play Again, Practice Again, Dashboard | Authoritative readmission or Dashboard |
| Settings | Implemented controls only; existing draft Apply/Cancel, reduced motion | Origin preserved; Dashboard access through Menu |
| Dashboard Menu | Settings, Credits, real Favorite toggle, Change Server, Quit, Back | Existing action/confirmation routes |
| Confirmation | Specific consequence, confirm/cancel, one focus owner | Return to invoking context; no generic extra warnings |
| Credits | Real attribution and Back | Invoking menu/context |
| Recovery | Specific actionable failure, supported retry/change-server route | Preserve accepted data; no false success |

All surfaces use the shared clean palette and font, actual data and one clear back route. Details outside the four approved compositions need native review or focused concept approval before a new visual layout is treated as accepted. Archived initial maps document research only.
