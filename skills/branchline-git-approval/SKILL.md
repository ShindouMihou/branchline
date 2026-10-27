---
name: branchline-git-approval
description: Request Branchline approval for commits, pushes, GitHub changes, destructive Git operations, and history rewrites. Safe local work and isolated test fixtures run directly.
---

Choose the execution path by the command's effects. Task authorization is still required; this guide defines when native Branchline approval is also needed.

## Run directly

Use normal tools for read-only Git/GitHub queries, builds, and tests. Safe local-only changes can also run directly: fetch without force/pruning, staging/unstaging without moving HEAD or discarding files, stash save/apply/pop with Git's normal conflict protections, and local branch creation, rename, or switching when existing work is preserved. Inspect first when safety depends on repository state. These operations do not need a Branchline request or an available MCP connection.

Tests may create commits, branches and stashes, rewrite history, and push to local bare remotes inside disposable test fixtures. Keep every effect confined to isolated fixture repositories and mock services: no real credentials, real remote services, user working repositories, or shared resources. A test or script that affects real repositories follows the approval rules below.

## Request Branchline approval

Use the connected MCP server for commits in real repositories; pushes and other remote writes; creating, updating, merging, closing, or otherwise changing pull requests and GitHub resources; history rewrites; deletion of branches/stashes or work; and commands that can discard or overwrite existing work. Examples include rebase, amend, reset that moves HEAD or discards files, clean, forced checkout, stash drop/clear, and force push. Changes to authentication, security settings, or publication destinations also require review.

Classify scripts, aliases, hooks, and shell chains by their actual effects. If a chain contains a gated action, submit the entire unchanged chain; approval never covers only a subset. Read-only inspection can resolve uncertainty. If the effects still cannot be established as safe local work, use Branchline. Preserve any stricter explicit user or repository requirement.

## Approval workflow

1. Read `branchline_capabilities` and choose an explicit absolute `repositoryPath`, or resolve a known repository with `branchline_repositories`. The app's visible repository is not the target selector. Use `branchline_inspect` for permitted local metadata/source inspection when needed.
2. Prepare the entire command for `branchline_propose_command`, using its current schema. Preserve a user-supplied command exactly; inspect and explain it rather than rewriting, splitting, removing flags, or approving a subset. When composing a command from the user's goal, keep every action within that goal. Git/gh combinations, flags, aliases, hooks, extensions, and shell operators are not restricted to a command catalog. The command runs through noninteractive `/bin/sh -c` as the user's account after approval; explain cross-repository targets, publication, deletion, history rewrites, or external execution when relevant. Commands needing a terminal/editor may fail or time out.
3. Provide a stable idempotency key for this exact attempt and a short plain-language explanation: outcome, reason, impact, and uncertainties. When composing a command, supply explicit commit messages and PR title, description, source branch, and base branch so Branchline can show structured details. Preserve an already supplied command even when its details are dynamic; explain the uncertainty instead. Name branch/remote/provider targets when known. Branchline displays the exact command and does not independently certify its effects. Keep credentials and secrets out of command arguments, input, and explanation: request data and local receipts are persisted. Authentication should use the user's existing configured credentials.
4. Call `branchline_request_review` once for the returned plan. Approval covers the whole unchanged command. The native user can approve or deny, optionally giving a reason; there is no partial approval or MCP approval tool. Tell the user the request is waiting. Submission, an open window, and elapsed time are not approval.
5. Call `branchline_wait_for_result` on the same `planId`. While `waitOutcome` is `timed_out`, wait again on that plan while the task remains active. Timeout or transport failure implies neither approval nor completion. If the user asks to stop waiting or the host cannot continue, report the last observed status and leave the request pending unless cancellation was requested. Use `branchline_cancel_plan` only when cancellation is intended; completed effects remain.
6. Report the actual status, process exit code, timeout, and recorded error. A successful command means the process exited successfully; `effectsVerified: false` means Git/GitHub/shell effects were not independently verified. Raw stdout/stderr is withheld from MCP because commands can print secrets. Do not invent command output or infer a PR URL. Include a PR URL only when a separate authorized inspection has established it or a verified result explicitly returns it. A shell command may partially change local or remote state before failing; inspect before retrying. Never retry an uncertain mutation automatically.
7. For a denial or `needs_attention`, report the recorded reason and stop dependent work. A requested revision needs a new exact proposal and a new native approval. If tools are unavailable, pause only actions that require approval and report the blocker; safe direct work can continue. Do not bypass a required review through another tool. A receipt only records the completed permission scope and cannot authorize another attempt.

A skill teaches this workflow; it does not sandbox an AI with separate terminal access. Installing the guide does not connect MCP automatically; complete Branchline's Connections setup first.
