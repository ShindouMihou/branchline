# Composable AI workflows

Superseded design history: the user selected one whole-command permission gate, including unchanged shell chains. The current contract is [exact command approval](MCP_CONNECTOR.md); typed-adapter modes and per-step approvals below are not the public implementation direction.

Status: proposed implementation design, 27 October 2026. This document expands the direction in [the connector specification](AI_CONNECTOR_SPEC.md); it does not add executable operations. [Command coverage](COMMAND_COVERAGE.md) remains authoritative for shipped behavior.

## Product contract

An AI should be able to submit a complete Git/GitHub workflow without first tailoring it to Branchline's small operation list. Branchline accepts the request for analysis, resolves each step, explains the effects, and reports exactly which steps it can execute. Submission, compatibility, approval, execution, and verification are separate states.

Broad request acceptance must not become a claim of universal execution. Git and gh include account administration, credentials, extension execution, interactive commands, and operations whose effects depend on external systems. These need different adapters and review representations. A command that cannot yet be verified stays visible with a specific reason and suggested alternative. It must not silently disappear, run through a shell fallback, or block independent work that the user separately approves.

The desired everyday flow is: create a branch → stage selected changes → commit → push → open a pull request → add reviewers. Other valid arrangements include fetch → compare → fast-forward; stash → switch → apply stash; cherry-pick several commits → push; and create a release → upload explicitly selected assets.

## Current constraints found in source

- `git/ai.rs::Step` has only commit, push, and PR variants. Its preview accepts at most three steps with fixed ordering, one commit, one destination, and a PR requiring an earlier matching push.
- A single `Preview` stores one HEAD, staged patch, push, and PR. It cannot describe evolving index/branch state or multiple targets.
- The broker propagates only a commit OID between steps. Approval grants a prefix by count; the UI renders specialized summaries for the three operations.
- MCP advertises at most three steps, while the broker's initial envelope check permits ten. The executor still rejects more than three. Increasing either limit alone adds no composability.
- The command catalog inventories 381 entries, but most are manual. Inventory coverage is not executable coverage.

## One request interface, two input forms

Add an additive `branchline_propose_workflow` tool using schema version 2. Keep version 1 plans, journal history, and tools working during migration. Continue using request-review, get-plan, cancel, and wait-for-result against the resulting plan identifier.

Each request carries the existing explanation and idempotency key, plus up to 32 ordered steps. Each step has a unique ID, repository selector, dependencies, input, and optional display intent. Repository selection may inherit from the request. Initially execute serially even for independent steps; dependency modeling is for correctness and approval, not speculative parallel execution.

Two mutually exclusive input forms normalize through the same compiler:

1. **Typed operation:** stable operation ID, revision, and validated parameters. This is the preferred form for skills and common workflows.
2. **Command proposal:** `program: git | gh` and an exact `argv` array. A parser translates recognized commands and flags to typed operations. It returns per-step compatibility feedback for everything else. This is an analysis interface, not an arbitrary command runner.

Never accept a shell string, arbitrary executable, environment overrides, shell interpolation, pipeline, redirect, or caller-controlled working directory. Choose the executable and canonical repository in Branchline. A recognized command name alone is insufficient: all flags, positional arguments, configuration overrides, output paths, transport behavior, and indirect execution must be covered by its adapter.

For example, `git add -- src/welcome.ts` normalizes to exact-file staging; `git push origin HEAD:refs/heads/feat/test` normalizes to a push with explicit source and destination. Unknown flags produce `unsupported_arguments`, with their positions and a suggested supported input. Never silently strip flags or reinterpret force as a normal push.

Proposed typed input shape:

```json
{
  "schemaVersion": 2,
  "repositoryPath": "/absolute/path/to/repository",
  "idempotencyKey": "publish-welcome-v1",
  "explanation": {
    "summary": "Publish the welcome message in a pull request.",
    "reason": "Let the team review the wording before merging.",
    "impact": "Create a branch and publish only the selected file.",
    "uncertainties": []
  },
  "steps": [
    {"id":"branch","operation":"branch.create","revision":1,"parameters":{"name":"feat/test","startPoint":"HEAD","checkout":true},"dependsOn":[]},
    {"id":"stage","operation":"index.stage","revision":1,"parameters":{"paths":["src/welcome.ts"]},"dependsOn":["branch"]},
    {"id":"commit","operation":"commit.create","revision":1,"parameters":{"message":"Improve welcome wording","indexFrom":{"stepId":"stage","output":"indexSnapshot"}},"dependsOn":["stage"]},
    {"id":"push","operation":"remote.push","revision":1,"parameters":{"remote":"origin","source":{"stepId":"commit","output":"commitOid"},"destinationBranch":"feat/test","mode":"create_or_fast_forward"},"dependsOn":["commit"]},
    {"id":"pr","operation":"pr.create","revision":1,"parameters":{"publication":{"stepId":"push","output":"publication"},"base":"main","title":"Improve welcome wording","body":"Clarifies the welcome message.","draft":true},"dependsOn":["push"]}
  ]
}
```

Output references are typed values, never textual substitutions. Reject cycles, forward dependencies, missing IDs, cross-repository OIDs, incompatible output types, and unbounded input sizes before preview. A publication output binds provider host, repository, destination ref, and verified SHA. A PR output binds provider resource identity and verified URL. Use canonical numeric/node identities where available rather than display labels alone.

## Compiler and executor seam

Introduce a workflow module with a small interface: analyze, prepare review, execute approved steps, and reconcile an interrupted attempt. Internal operation adapters implement validation, effect projection, preconditions, command preparation, execution, verification, and reconciliation. The catalog and MCP capabilities are generated from this operation registry, so discovery cannot claim support absent from execution.

Pipeline:

1. Validate the request envelope and identities without mutation or surprise network access.
2. Normalize typed inputs or recognized argv into versioned operations.
3. Resolve dependencies and project repository/provider state through each step.
4. Produce per-step effects, evidence, expected outputs, risks, and review checkpoints.
5. Freeze the normalized plan and evidence under an approval digest.
6. After native approval, revalidate preconditions and persist command intent before every side effect.
7. Verify postconditions, persist outputs, and release only verified values to dependent steps.

Keep plan orchestration out of Git primitives. Split new modules under `ai/workflow/` into input types, compilation, registry, approval, execution, and receipts; keep Git and GitHub adapters alongside their respective operation primitives. Frontend shared types belong in the existing types location. Avoid adding more special cases to the large broker or UI renderer.

Every adapter declares its read set, write set, network destinations, privacy needs, effect representation, retry class, conflict behavior, and supported CLI/version constraints. Probing CLI capabilities must be bounded and must not execute aliases, extensions, hooks, or authentication changes.

## State projection and review checkpoints

Each step has its own before/after state. Earlier steps may change branch, index, HEAD, refs, remote state, or provider resources; later steps validate against those approved transitions rather than the initial snapshot.

- **Staging:** bind literal repository-relative paths, current index, exact reviewed file content/mode/deletion state, and resulting index tree. No wildcard expansion, implicit `add .`, ignored-file force addition, or accidental inclusion of other staged changes. If other staged content will enter the commit, display it explicitly or require an isolated selection. Use an isolated temporary index for previews; recheck the real index and selected file content before mutation. Filters, symlinks, submodules, partial files, and index extensions require explicit handling rather than a raw file-hash shortcut.
- **Branch creation/switching:** resolve start point to an OID, bind branch absence or prior value, worktree ownership, and dirty-file impact. Do not add implicit stashing.
- **Fetch/pull:** network inspection is explicit. If fetched data changes the material effect of a downstream merge/rebase/pull, stop at a new review checkpoint. Approval of an unknown future diff is not approval of every possible fetched result.
- **History operations:** bind selected OIDs and affected descendants, publication evidence, backup refs, and conflict policy. Conflicts stop the workflow; never automatically resolve or continue under an obsolete diff approval.
- **Provider actions:** bind host, owner/repository, authenticated account, resource ID, current revision, and exact content/visibility. PR creation can target an already-published branch without a mandatory push, once verified.
- **Dynamic outputs:** a newly generated commit OID may be consumed without another review only when it satisfies the approved parent/tree/message constraints. Unexpected changes, extra files, destinations, recipients, or publication scope invalidate downstream approval.

Unknown future effects create a checkpoint, not an automatically approved placeholder. Read-only remote checks also disclose information and may run credential/transport helpers; their policy belongs to the adapter.

## Approval and simple presentation

Show one compact timeline with a title, operation icon, target, and short effect summary per step. Keep commands, detailed diffs, and technical evidence expandable. AI-provided reasons are separate from Branchline's computed effects.

Use distinct compatibility labels: `verified`, `checkpoint_required`, `unsupported_arguments`, `adapter_required`, `policy_blocked`, and `environment_unavailable`. A verified preview is still unapproved. Give the AI machine-readable reasons and suggested replacements so it can revise the request without guessing.

Native approval binds explicit step IDs plus all required dependencies, normalized arguments, output constraints, adapter revisions, repository identities, evidence, and risk acknowledgements. Replace prefix-only authorization for version 2. Unchecking a prerequisite also disables its dependents; the UI explains why. Never run an independent supported subset merely because analysis succeeded: the user approves that subset explicitly.

Keep primary-branch and mismatched-destination warnings, and extend them to history rewrites, force updates, file deletion, remote deletion, public visibility, release publication, notifications, credential/permission changes, workflow execution, and paid resources. A risk warning is not a substitute for an adapter. Destructive actions need their own preview and acknowledgement, not a universal red checkbox.

## Coverage rollout

All entries below are proposed; existing shipped scopes remain documented separately.

| Delivery | Families | Useful combinations |
|---|---|---|
| 1. Workflow foundation | Versioned registry, per-step snapshots, typed outputs, dependency approvals, exact-file stage/unstage, branch create | Branch → stage → commit → push → PR; stage multiple groups into separate commits |
| 2. Everyday Git | Branch switch/rename, fetch, fast-forward integration, tags, stash save/apply, bounded inspection | Fetch → compare → update; stash → switch → apply |
| 3. GitHub collaboration | PR view/edit/reviewers/labels/comment/ready/close/reopen; issue create/edit/comment/link; checks | Publish → PR → request review; issue → branch → PR |
| 4. History and conflicts | Revert/cherry-pick/merge/rebase/squash/reword/reset; resolve/continue/abort with checkpoints | Cherry-pick → inspect → publish; rewrite → review → push with exact lease policy |
| 5. Releases and automation | Releases/assets, workflow dispatch/rerun/cancel, artifact download | Tag → push tag → draft release → upload reviewed assets |
| 6. Advanced scopes | Worktrees, submodules, sparse/LFS, repository settings, scoped REST/GraphQL adapters, Enterprise | Cross-repository workflows with separate identity and approval scopes |

No blanket “all Git/gh commands supported” claim. Credentials, repository deletion, access administration, custom hooks, aliases, extensions, and remote compute are separate capabilities, not ordinary publication flags. Initially unsupported commands remain analyzable requests with a reason; executing opaque custom code requires a separately designed product policy and cannot inherit ordinary Git approval.

## Receipts, waiting, and failures

Retain the existing rule: timeout never counts as approval or completion. Return per-step compatibility and execution states, verified outputs, checkpoint information, and the next action. A waiter must distinguish waiting for approval, running, awaiting a new review, and terminal failure/partial completion.

Journal each attempt with plan/step IDs, client/provider and self-reported model distinction, repository identity, approved digest, adapter/CLI versions, redacted exact argv, input/content digests, start/end timestamps, outcome, and verification evidence. Credential values and unapproved source content never enter the receipt. Display command redaction explicitly. Persist intent before launching; failure to persist prevents launch.

Stop on failure or uncertainty. Preserve completed effects and never describe the batch as atomic. Do not auto-retry push, PR/comment creation, releases, workflow dispatch, or deletions after a lost response. Reconcile by exact identity and approved effects, then request a new attempt when certainty is unavailable. Local idempotency keys prevent duplicate submissions; they do not make remote providers universally idempotent.

Cancellation takes effect at safe command boundaries. Terminating a process does not establish that its remote mutation failed. Lock canonical repositories/common Git directories in deterministic order for conflicting workflows; provider locks use host/resource identity. Revalidate against external processes before each step. Never hold the UI/broker mutex throughout a subprocess or wait loop.

## Required acceptance coverage

- Real temporary repositories: stage → commit with exact selected tree; branch creation before commit; multiple staged groups; deletion/rename/binary/symlink cases; preservation of unrelated staged and working content.
- No mutation during compilation; unknown argv, unsupported flags, config overrides, aliases, path traversal, pathspec magic, shell-looking filenames, cross-worktree changes, and stale fingerprints fail closed.
- Multi-step effects: missing/cyclic dependencies, invalid output references, repeated operation kinds, multiple destinations, explicit subset approval, changed dependencies, checkpoint re-review, and legacy version 1 compatibility.
- Recovery: cancellation at every boundary, revoked connection, expired approval, journal write failure, crash after intent, crash after mutation, lost network response, duplicate proposal, and no implicit retry.
- Provider fixtures: PR on an existing published branch, duplicate PR/comment ambiguity, wrong host/account/repository/SHA, rate limits, expired auth, partial workflow success, verified URL propagation, and logs without secrets. Hosted mutation tests require an explicitly designated test repository.
- Native UI: glanceable long workflows, unsupported-step feedback, dependency selection, risk alignment, scroll preservation, cross-repository routing, and actual wait results. Existing request-review focus behavior remains unchanged.

## External behavior checked

Git documents that configuration and hooks can execute arbitrary shell commands: [Git reference, security](https://git-scm.com/docs/git#_security). Therefore spawning a process without a shell is necessary but insufficient for safe Git approval.

GitHub CLI's [`gh api`](https://cli.github.com/manual/gh_api) can address REST and GraphQL and changes its default method when fields are supplied. It needs endpoint/method/field-specific normalization; it must not be assumed read-only from the command name. [gh extensions](https://cli.github.com/manual/gh_extension) add executable commands and cannot inherit built-in operation verification.
