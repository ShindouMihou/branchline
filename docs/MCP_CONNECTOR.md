# AI Connector: exact command approval

Implementation direction updated 27 October 2026. New requests use one exact-command permission gate. The earlier typed commit/push/PR design remains relevant to historical receipts, not a second public execution mode. See [command coverage](COMMAND_COVERAGE.md) for scope and [validation](../VALIDATION.md) for checks actually performed.

## Connect once

Open **AI requests → Connections**, enable the connector, and choose **Add another AI**. Setup has two steps:

1. Choose Claude Code, Codex, Cursor, Claude Desktop, or another local MCP client, and name the connection.
2. Copy the generated setup command or configuration. Where supported, choose **Across projects** or **This project**. Follow the verification steps in the guide.

Client classification and icons are stored with the connection. Existing connections can change their client type without changing credentials; **Setup** reopens the guide for the same connection. Classification is user-selected, not cryptographic proof of the actual application or model.

The guide never silently edits an AI client's configuration or runs an installation command. Its paths point to the current packaged executable and a private credential file. Configuration contains a credential-file path, not the raw token. Keep the file private.

### Client-specific setup

- **Claude Code:** the generated stdio command uses `--scope user` across projects or `--scope local` for the current project. Check with `claude mcp get branchline` or `/mcp`. [Official instructions](https://code.claude.com/docs/en/mcp).
- **Codex:** global setup uses `codex mcp add branchline -- <command>`. Project setup supplies a `[mcp_servers.branchline]` section for `.codex/config.toml`; Codex loads project configuration in trusted projects. The app, CLI, and IDE share user-level configuration. [Official instructions](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).
- **Cursor:** merge the generated entry into `~/.cursor/mcp.json` globally or `.cursor/mcp.json` in a project. Keep other server entries. [Official instructions](https://cursor.com/docs/mcp).
- **Claude Desktop:** use Settings → Developer → Edit Config, merge the entry, then quit and reopen the client. On macOS the configuration is `~/Library/Application Support/Claude/claude_desktop_config.json`. [Local-server guide](https://modelcontextprotocol.io/docs/develop/connect-local-servers).
- **Another client:** use the generated local stdio configuration in that client's MCP settings.

Instructions were checked against current official documentation and installed Codex/Claude CLI help. Existing host settings were not modified, and live interoperability with every listed client has not been certified.

## Agent routing scope

The skill, managed instruction blocks, MCP initialization guidance, tool descriptions and capabilities scope Branchline to Git/GitHub effects. They are routing guidance; the shell executor is not a static sandbox and does not prove that arbitrary scripts only affect Git. Existing AI sessions and MCP connections need to reload guidance; bundled transport changes become available with an updated app build.

| Request | Intended route |
| --- | --- |
| Convex `env default set`, including preview deployments | Convex's normal tools and permissions |
| Database deletion, cloud credential changes, or package publication | The service's normal tools and permissions |
| `git status`, `gh pr view`, staging, safe stash work | Normal tools |
| Isolated fixture tests creating commits or local bare-remote pushes | Normal tools, confined to disposable fixtures |
| Git commit/push through Git, an alias, Node or another script | Branchline |
| GitHub PR edits, release creation, repository-security changes | Branchline |
| Git remote URL or Git/GitHub authentication changes | Branchline |
| Unknown script with unknown effects | Inspect or clarify with normal tools; no automatic Branchline fallback |
| `git status && convex env ...` | Normal tools and Convex authorization; read-only Git adds no gate |
| `git push && local-build-check` | Whole unchanged Git workflow through Branchline |
| User-supplied `git push && convex env ...` | Clarify separation and permissions before splitting/changing or submitting wholesale |

This review addresses the previous ambiguous terms “remote writes,” “security changes,” and “deletion” by qualifying them as Git/GitHub effects. Non-Git risk remains subject to the appropriate service's authorization; it is not silently exempted. The boundary preserves safe-local and fixture exceptions, whole-command Git approval, and waiting for actual results.

## One command, one decision

An AI submits the entire Git/GitHub command using `branchline_propose_command`. Branchline preserves the text and presents it for native approval. It does not split the command, remove flags, silently substitute safer commands, or approve an initial portion. The user approves the entire command or denies it, optionally giving a reason visible to the AI.

The command executes as `/bin/sh -c <exact text>` with the selected repository as its initial working directory. This supports arbitrary installed Git and GitHub CLI subcommands and flags, and shell combinations such as `&&`, pipelines, conditionals, redirects, and loops. Shell semantics determine ordering: `&&` stops on a failed left side, while `;` continues. The final process exit code is not proof every earlier shell subcommand succeeded.

This is unrestricted user-account command permission. A command can change directory, address another repository or GitHub host/account, launch hooks/helpers/extensions, access files, or run other programs. The selected repository is context, not a sandbox. Exact-command approval is broader than the former typed publication approval. The review requires explicit acknowledgement that effects are not independently verified and that external/user-account actions are possible.

The app binds its system shell executable and initial working-directory identity to the review. It cannot bind every executable later selected by shell PATH resolution, aliases, scripts, Git configuration, or external helpers. Risk highlights are assistance, not an exhaustive command analyzer or a guarantee of safety.

## Connection and repository context

Connections are global. Pass an absolute `repositoryPath`; known IDs from `branchline_repositories` are a convenience. Metadata includes paths, branches, commit messages/authors, and staged-file names. Optional source-diff access remains a separate connection preference. Ordinary inspection does not approve execution or open a review prompt.

A cold MCP connection may start the app hidden in the background. `branchline_request_review` raises the matching repository window when automatic prompts are enabled, which is the default. The window opening never grants approval. Revocation/disablement prevents new approval; cancellation takes effect at command boundaries and cannot recall completed or already-sent effects.

## MCP tools

| Tool | Purpose |
|---|---|
| `branchline_capabilities` | Current exact-command contract, limits, and explanation guidance |
| `branchline_command_catalog` | Installed-command inventory; not an execution allowlist |
| `branchline_repositories` | Known repository contexts |
| `branchline_inspect` | Local status/history and permitted staged source diff |
| `branchline_propose_command` | Prepare one unchanged shell command; never executes |
| `branchline_request_review` | Queue native whole-command review |
| `branchline_wait_for_result` | Await denial, execution result, or attention; timeout never approves |
| `branchline_get_plan` | Read owned request, command receipts, status, and denial reason |
| `branchline_cancel_plan` | Stop unstarted work; completed effects remain |

Example proposal arguments, not a JSON-RPC envelope:

```json
{
  "schemaVersion": 1,
  "repositoryPath": "/absolute/path/to/repository",
  "idempotencyKey": "publish-welcome-01",
  "command": "git add -- src/welcome.ts && git commit -m 'Improve welcome wording' && git push origin HEAD:refs/heads/feat/test && gh pr create --head feat/test --base main --title 'Improve welcome wording' --body 'Clarifies the welcome message.'",
  "explanation": {
    "summary": "Publish the welcome change for review.",
    "reason": "Let the team review the wording before merging.",
    "impact": "Stage the selected file, create a commit, push feat/test, and open a pull request.",
    "uncertainties": ["Authentication, hooks, and remote branch policy may stop the command."]
  }
}
```

This example is explanatory and must not be run as a test on a user repository. Staging/committing follows the actual command and Git's current index; `git commit` can include files already staged before the command. The review does not independently calculate or limit its tree.

Idempotency binds the complete input. Reusing a key with a changed command is rejected. Unexpected fields and forged approval data are rejected. Existing old receipts remain readable; historical typed steps retain their historical verification evidence.

## Execution, waiting, and output

The runner is noninteractive: no terminal, pager, editor, or interactive GitHub prompt. Use commands that supply required input through flags or optional stdin. Installed tools, configured authentication, permissions, CLI behavior, and provider policy determine whether a particular command succeeds. Interactive/editor-dependent operations may fail or time out rather than being universally compatible.

Default command timeout is 120 seconds, configurable from 1 to 600 seconds. The local transport request frame is limited to 1 MiB including its JSON envelope; parameter limits do not override the frame limit. Output capture is bounded to 128 KiB per stream with truncation indicators. Timeout terminates the launched process group on a best-effort basis; detached or remote effects may remain. Approval expires after ten minutes and is not restored after restart.

Call the bounded wait tool repeatedly on the same plan after requesting review. Its default is 20 seconds and maximum 25 seconds; those waits do not alter command timeouts. `timed_out` means still waiting. `succeeded` for command execution means the process exited successfully. `effectsVerified: false` explicitly states that Git, GitHub, hook, helper, shell, and external effects have not been independently verified. Failed or timed-out commands may have partially completed effects; there is no automatic rollback or retry.

Raw stdout/stderr may contain tokens, source, credentials, or other private data. They are withheld from MCP responses and ordinary command receipts; captured-byte counts, digests, truncation flags, exit code, and timing remain available. Consequently an AI cannot obtain a raw `gh pr create` URL merely from this command result. It must report that limitation or use a separately authorized inspection. Never label an arbitrary URL verified.

## Activity and privacy

Receipts retain connection name/provider, self-reported model distinction, exact requested command/argv, optional input binding, repository context, approval digest and time, risk acknowledgement, denial reason, command timing, exit status, timeout, and unverified-effect status. Native approval is a local user decision, never an MCP token.

Request text, arguments, and stdin can contain secrets even when stdout is withheld. They are stored locally for exact review/audit and may be present in exports. Avoid embedding secrets; use ordinary credential helpers and environment-based authentication already configured by the user. The journal is private local JSON, not encrypted or tamper-proof storage. Clipboard/export is a user action and does not itself transmit to a model provider.

The broker uses per-registration credentials, same-user peer checks, a 0700 directory and 0600 files/socket. These controls do not sandbox the approved command or isolate a privileged same-user process. The execution gate serializes Branchline requests, not external Git processes. Current journal limits remain 100 requests per registration and 50 registrations.

### ChatGPT desktop setup

Choose **ChatGPT Desktop** in Connections. In ChatGPT, open **Settings → MCP servers → Add server**, name it Branchline, and select **STDIO**. Copy the displayed command and add each argument as its own entry. Save, select Restart, then start a new local chat and use `/mcp` to verify the connection. No Codex CLI installation is required. This connection uses the local desktop host.

Official instructions: https://learn.chatgpt.com/docs/extend/mcp

### Background startup

A cold MCP connection launches the packaged application with `--mcp-background`. The main window is created hidden and unfocused before startup, so inspection and proposal calls do not show it. An explicit review request reveals and focuses the window when automatic review prompts are enabled. Opening Branchline from the Dock/Finder also reveals the background window.


### Background work and time limits

Execution waits for the approved shell process. A successful exit does not prove that background jobs finished or that every Git/GitHub effect succeeded. A timeout terminates the shell process group, but deliberately detached processes can escape that group and continue. Cancellation does not undo completed actions. Inspect local and remote state before retrying an interrupted request; never treat a timeout as approval or as proof that all work stopped.

### Clearing local request activity

The Activity tab includes a native-only Clear activity action with confirmation. It deletes finished and blocked request records across connections, including their local approval/command receipts. Prepared, pending-review, approved and running requests remain. Git changes, connection credentials and previously exported files are unchanged. Cleared request IDs can no longer be inspected or waited on; reusing an old idempotency key creates a new request requiring a new native approval. No MCP tool can clear this activity.

Needs you lists only requests pending an approval decision. Blocked, stopped, finished, prepared and running requests appear in Activity; historical blocked plans need a new AI request rather than a manual approval.

### Preview changes and file-backed PR descriptions

Command warnings and action cards share one bounded shell analysis. Preview parsing uses deterministic work limits, so system load does not decide which actions appear. Approval still rechecks the preview. If it differs, Branchline lists what changed (such as a warning, branch, file selection, or PR description) in the stopped request and activity record. Nothing executes; the AI must submit a fresh request.

`gh pr create --body-file PATH` and `-F PATH` display the current Markdown description from a regular UTF-8 file up to 64 KB. Absolute paths, including temporary files outside the repository, can be previewed after runtime guards. Relative paths require a known repository context. Missing, symbolic-link, special, oversized, or non-UTF-8 files produce an explanation instead. For a single literal `gh pr create` or `gh pr edit` command, `--body-file -` previews the supplied standard input as Markdown (up to 64 KB). Empty input is distinguished from input that was not supplied. In chains, pipelines, and redirects, input is not attributed to a PR because an earlier command may consume or replace it. Runtime-generated descriptions are not inferred. The body is a current snapshot: earlier commands or later edits can change what GitHub CLI eventually reads. Changes detected at approval require review again; commands still run unchanged.

`gh pr edit NUMBER_OR_URL_OR_BRANCH` has a structured update card showing its literal PR target, explicit `--repo`/`-R`, new title, requested base change, and description. It displays only requested fields and does not infer a source-branch change or fetch the existing PR. Exact stdin remains available in a quiet disclosure below the card; ordinary activity receipts continue to omit raw stdin.

## Optional user-configured automatic approval

Manual review remains the default. Native settings can grant connection/repository-scoped, session-limited automatic approval for selected operations or all requests. MCP cannot configure these rules. Agents still submit the whole unchanged request, request review, and wait for the actual result. Receipts identify automatic authorization as `native_policy` and include a rule snapshot; a timeout grants nothing. See [automatic approval](AI_AUTO_APPROVAL.md) for syntax coverage, pause behavior and lifetime.
