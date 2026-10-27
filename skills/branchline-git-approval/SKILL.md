---
name: branchline-git-approval
description: Request approval for consequential Git and GitHub operations only, including Git effects invoked through scripts. Unrelated services use their normal tools and permissions; safe local Git work and isolated fixtures run directly.
---

## Scope first

Branchline is strictly the approval gate for Git and GitHub operations. Determine the actual effects before choosing a tool; a repository working directory or a remote write alone does not make a command a Git/GitHub operation.

Use unrelated services' normal tools and permission mechanisms for their operations. This includes Convex environment changes, application deployments, database changes, cloud administration, and non-GitHub authentication or security changes. They stay outside Branchline even when remote, destructive, or security-sensitive. This rule grants no permission to perform them directly without their normal authorization.

Inspect scripts, aliases, hooks, and wrappers by their effects: a Node script that commits or pushes Git needs the Git approval below; a Node command that sets a Convex variable does not. If the service/effects are unknown, inspect or ask for clarification using normal tools. Branchline is not a fallback for unclassified commands.

## Safe Git work

Read-only Git/GitHub queries, builds, tests, and safe local Git work use normal tools. Examples include fetch without force/pruning, staging/unstaging without moving HEAD or discarding files, stash save/apply/pop with normal conflict protections, and local branch creation/rename/switching that preserves existing work. Inspect repository state when safety depends on it. No Branchline connection is needed for this work.

Tests may create commits, branches and stashes, rewrite history, and push to local bare remotes inside disposable fixtures. Confine every effect to isolated fixture repositories and mock services: no real credentials, real remote services, user working repositories, or shared resources. Git/GitHub effects outside those fixtures follow the approval rules below.

## Gated Git/GitHub work

Use Branchline for commits in real repositories, Git pushes and other Git-remote writes, GitHub resource changes (including PRs, releases and repository settings), Git history rewrites, Git branch/stash/worktree deletion, and Git operations that discard or overwrite work. Examples include rebase, amend, reset that moves HEAD or discards files, clean, forced checkout, stash drop/clear, and force push. Git/GitHub authentication, repository-security settings, and Git remote/publication-destination changes also require review. If a known Git/GitHub action cannot be established as safe local work, use Branchline. Preserve stricter explicit user or repository requirements within that scope.

## Command chains

Keep Git/GitHub workflow chains intact, including supporting shell setup, read-only checks, and local build/test steps. A gated Git/GitHub action requires approval of the entire unchanged chain; never approve only a subset.

When composing a workflow, keep unrelated service mutations in separate normal-tool operations with their own authorization. For a user-supplied chain mixing gated Git/GitHub work with unrelated service mutations, explain the scope conflict and ask before splitting or changing it. Do not silently rewrite it or submit it wholesale as a general-purpose Branchline approval. Branchline approval cannot authorize another service's operations.

## Approval workflow

1. Read `branchline_capabilities` and choose an explicit absolute `repositoryPath`, or resolve a known repository with `branchline_repositories`. The app's visible repository is not the target selector. Use `branchline_inspect` for permitted local metadata/source inspection when needed.
2. Prepare the entire command for `branchline_propose_command`, using its current schema. Preserve a user-supplied command exactly; inspect and explain it rather than rewriting, splitting, removing flags, or approving a subset. When composing a command from the user's goal, keep every action within that goal. Git/gh combinations, flags, aliases, hooks, extensions, and shell operators are not restricted to a command catalog. The command runs through noninteractive `/bin/sh -c` as the user's account after approval; explain cross-repository targets, publication, deletion, history rewrites, or external execution when relevant. Commands needing a terminal/editor may fail or time out.
3. Provide a stable idempotency key for this exact attempt and a short plain-language explanation: outcome, reason, impact, and uncertainties. When composing a command, supply explicit commit messages and PR title, description, source branch, and base branch so Branchline can show structured details. Preserve an already supplied command even when its details are dynamic; explain the uncertainty instead. Name branch/remote/provider targets when known. Branchline displays the exact command and does not independently certify its effects. Keep credentials and secrets out of command arguments, input, and explanation: request data and local receipts are persisted. Authentication should use the user's existing configured credentials.
4. Call `branchline_request_review` once for the returned plan. Approval covers the whole unchanged command. The native user can approve or deny, optionally giving a reason; there is no partial approval or MCP approval tool. Tell the user the request is waiting. Submission, an open window, and elapsed time are not approval.
5. Call `branchline_wait_for_result` on the same `planId`. While `waitOutcome` is `timed_out`, wait again on that plan while the task remains active. Timeout or transport failure implies neither approval nor completion. If the user asks to stop waiting or the host cannot continue, report the last observed status and leave the request pending unless cancellation was requested. Use `branchline_cancel_plan` only when cancellation is intended; completed effects remain.
6. Report the actual status, process exit code, timeout, and recorded error. A successful command means the process exited successfully; `effectsVerified: false` means Git/GitHub/shell effects were not independently verified. Raw stdout/stderr is withheld from MCP because commands can print secrets. Do not invent command output or infer a PR URL. Include a PR URL only when a separate authorized inspection has established it or a verified result explicitly returns it. A shell command may partially change local or remote state before failing; inspect before retrying. Never retry an uncertain mutation automatically.
7. For a denial or `needs_attention`, report the recorded reason and stop dependent work. A requested revision needs a new exact proposal and a new native approval. If tools are unavailable, pause only actions that require approval and report the blocker; safe direct work can continue. Do not bypass a required review through another tool. A receipt only records the completed permission scope and cannot authorize another attempt.

A skill teaches this workflow; it does not sandbox an AI with separate terminal access. Installing the guide does not connect MCP automatically; complete Branchline's Connections setup first.
