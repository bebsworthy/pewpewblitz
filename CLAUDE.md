@AGENTS.md

<!-- ticket:skill:begin (v4) -->
## Ticket — task tracker

This project uses `ticket` (CLI). Track work there, not in chat:
`ticket task <ID>` to read · `ticket task new <title>` to create ·
`ticket task <ID> update` (stdin JSON) to change · ask humans via
`ticket task <ID> question add` (a markdown heredoc on stdin; `question
template` shows the form). Markdown by default, `--json` if you must parse.
Full help: `ticket --help` and every subcommand.
When the human agrees a mockup, diagram or other artifact, attach it in
the same turn (`ticket task <ID> asset add <file> --caption …`) and link
it from the spec.
Your writes are recorded as `agent:<name> for <your human>`: set
`TICKET_AGENT=<name>` if you are not detected or share a checkout.
<!-- ticket:skill:end -->
