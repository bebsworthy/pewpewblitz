# Acceptance criteria

- AGENTS.md explicitly states that Brawler no longer creates big numbered versions or milestone roadmaps for active progress tracking.
- New work starts with a Ticket CLI task; its description and spec own scope, decisions, status, acceptance criteria, verification, playtest feedback, and learning.
- Ticket status remains honest from todo/backlog through doing and done; material scope changes are recorded before implementation.
- Human questions use Ticket's question workflow; ticket mirrors are never edited by agents and `ticket sync` is required before handoff.
- V1-V12 implementation documents remain historical evidence and durable product/technical docs remain authoritative.
- Existing implementation and verification safety rules refer to the active ticket rather than the active milestone/version backlog.
- Documentation diff passes `git diff --check` and the ticket is moved to done and synced.


# Implementation evidence

- Added Ticket as the first source for current-work orientation.
- Explicitly retired big numbered versions and milestone roadmaps as active planning/status mechanisms.
- Preserved V1-V12 as historical evidence and classified later drafts as non-authoritative context.
- Replaced the version/milestone process with the Ticket CLI lifecycle, including search/reuse, description/spec ownership, honest statuses, questions, deferred tickets, verification, playtest feedback, learning, done criteria, and sync.
- Updated protocol-change, vertical-slice, deferred-work, and implementation-scope rules to refer to the active ticket.
- `git diff --check` passed; no source or unrelated user file was changed for this ticket.


# Follow-up feedback

- State that Ticket is a new application and agents should report defects and concrete workflow or usability improvements discovered while using it.
- Ticket application issues belong to the registered `TCK` project, not Brawler's product backlog.
- Brawler application work remains in the current `BRL` project.
