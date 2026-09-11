# Player shell screen map

Current behavior/design contract. Approved composition details live in [README](README.md), and
shared component/state rules live in the [UI toolkit](ui-toolkit.md). Existing source and
[Player UX](../../13-player-ux.md) own server authority and lifecycle.

| Surface | Purpose and essential controls | Destination / behavior |
|---|---|---|
| Connecting | Real logo, live connection phase, Cancel | Accepted lobby → Dashboard; cancellation → Server Select; failure → full-screen Recovery |
| Server Select | Real logo, address and player identity, favorites/recents, Connect, Settings, Quit | Routed connection; no local/offline gameplay substitute |
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
| Settings | Category/options/details layout with implemented controls, auto-save preferences, scoped Reset/Undo, and explicit display preview confirmation | Origin preserved; Dashboard access through Menu |
| Dashboard Menu | Settings, Credits, real Favorite toggle, Change Server, Quit, Back | Existing action/confirmation routes |
| Confirmation | Specific consequence, confirm/cancel, one focus owner | Return to invoking context; no generic extra warnings |
| Credits | Real attribution and Back | Invoking menu/context |
| Recovery | Full-screen factual connection failure with Retry, Change Server, and Quit; no decorative status icon | Preserve accepted data; no false success or offline substitute |

All surfaces use the shared clean palette, type, semantic surface roles, and interaction states;
they present actual data and one clear back route. Details outside the approved compositions need
native review or focused concept approval before a new visual layout is treated as accepted.
Archived initial maps document research only.
