#!/usr/bin/env bash
# Lint exceptions must be narrow and reviewable (AGENTS.md "Code organization rules"): reject any
# crate-, module-, or file-wide inner `#![allow(...)]` / `#![expect(...)]`, including through
# `#![cfg_attr(..., allow(...))]`. Attach the exception to the item, statement, or `use` it covers.
set -euo pipefail

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

command -v rg >/dev/null || { printf '%s\n' 'ripgrep (rg) is required' >&2; exit 2; }

status=0
rg -n --glob '*.rs' \
    '#!\[\s*(allow|expect)\s*\(|#!\[\s*cfg_attr\s*\(.*\b(allow|expect)\s*\(' \
    src packages tests || status=$?
case "$status" in
    0)
        printf '%s\n' 'file-wide lint exception found; use an item-level #[expect(..., reason = "...")]' >&2
        exit 1
        ;;
    1) ;;
    *) exit "$status" ;;
esac

printf '%s\n' 'no crate-, module-, or file-wide lint exceptions in src, packages, or tests'
