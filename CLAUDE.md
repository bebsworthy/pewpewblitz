@AGENTS.md

<!-- ticket:skill:begin (v2) -->
## Ticket — task tracker

This project uses `ticket` (CLI). Track work there, not in chat:
`ticket task <ID>` to read · `ticket task new <title>` to create ·
`ticket task <ID> update` (stdin JSON) to change · ask humans via
`ticket task <ID> question add` (a markdown heredoc on stdin; `question
template` shows the form). Markdown by default, `--json` if you must parse.
Full help: `ticket --help` and every subcommand.
<!-- ticket:skill:end -->
