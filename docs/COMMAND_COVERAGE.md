# Git and GitHub command coverage

Updated 27 October 2026. The public request interface is one unchanged-command approval gate: `branchline_propose_command`. A user approves the whole command or denies it. The old typed publication adapters are retained for historical/internal compatibility, not advertised as an alternate request mode.

## Execution coverage

There is no Git/GitHub subcommand or flag allowlist in the command gate. The entire text runs through noninteractive `/bin/sh -c` after native approval. Installed Git/gh commands, arbitrary combinations, shell operators, aliases, extensions, configuration, and hooks follow their actual command-line behavior. Stage → commit → push → PR is one example rather than a fixed workflow.

This broad execution permission also permits other shell programs, filesystem access, and actions outside the initial repository. Branchline prominently requests acknowledgement of this scope. The initial repository selector does not confine effects. Unknown/new commands are not rejected merely because they are absent from the command catalog.

Command availability is not universal success. Installed versions, dependencies, authentication, terminal requirements, external tools, provider permissions/policies, and timeouts still apply. Interactive editors and terminal-driven commands are not supported interactively. A successful final process exit is recorded; arbitrary effects are **not independently verified**. Shell operators govern internal failure behavior, so an earlier command can fail while a later command produces an exit code of zero.

## Discovery and verification

The machine-readable inventory remains `src-tauri/src/ai/command-catalog.json`, exposed through `branchline_command_catalog` with optional `program` and `command` filters. Its 381 built-in entries were captured on 2026-10-26. Historical per-entry typed/manual scopes describe the former adapters; the catalog response must separately identify exact-command approval as the current general execution path. Inventory absence does not deny command execution.

Coverage dimensions must remain separate:

| Dimension | Meaning |
|---|---|
| Request accepted | Envelope and exact command are valid; no action has run |
| Approved | Native user approved the whole unchanged command and risk scope |
| Process completed | Exit code/timeout recorded under that approval |
| Effects verified | Not provided for arbitrary command requests |
| Output disclosed | Raw stdout/stderr withheld from MCP because it can contain secrets |

Read-only repository inspection remains typed and bounded, with optional source-diff access. It does not execute the submitted command or imply permission. Historical typed receipts can still contain verified commit/remote/PR evidence; that evidence must not be attributed to new shell requests.

## Limits and recovery

See [the connector contract](MCP_CONNECTOR.md) for the request shape, frame limits, timeouts, receipt privacy, and wait behavior. Approval never follows from a timeout. Native denial reasons are returned to the AI. A failed/uncertain command can have completed local or remote effects; inspect before retrying. There is no implicit retry, rollback, partial approval, or automatic rewrite.

The earlier [composable-workflow proposal](COMPOSABLE_AI_WORKFLOWS.md) describes an alternative typed-adapter roadmap. Its no-shell recommendation and per-family manual-only rollout were superseded by the user's explicit choice of one broad exact-command permission gate. It is retained as design history, not the current runtime contract.

References: [Git command reference](https://git-scm.com/docs/git), [GitHub CLI reference](https://cli.github.com/manual/gh_help_reference).
