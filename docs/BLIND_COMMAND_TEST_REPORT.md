# Blind command test report

Date: 2026-10-27

Three independent agents chose executor, preview, and permission-gate cases without receiving one another's case lists. A separate integration pass exercised the public connector's whole-command workflow and the packaged stdio process. Tests ran in disposable repositories and private broker instances. No live user request was approved, no user repository was modified, and no network mutation occurred.

## Initial blind pass

| Lane | Additional regression tests | What ran |
| --- | ---: | --- |
| Executor | 9 | Literal metacharacter filenames, compound operators, subshells, pipelines, heredocs, substitutions, partial effects, binary stdin, concurrent bounded output, timeouts, background jobs, audit write failures |
| Structured previews | 7 | 14 chosen workflows covering literal and dynamic arguments, commit messages/path selection, PR title/body/head/base, option separators, branch flags, comments, changed directories and environment prefixes |
| Permission gate | 6 | Public MCP proposals, pending waits, denial reasons, ownership, forged credentials/digests, command immutability, duplicate callbacks/approvals, schema boundaries, audit privacy and terminal-result preservation |
| Compound workflow | 2 | Real local stage/commit/branch/push followed by an offline gh stand-in; success and PR-command failure after the completed push |

At the end of the initial pass, the complete Rust suite passed **112 tests** (88 existing + 24 new), and the frontend suite passed **44 tests**.

The packaged executable also passed stdio checks for all three supported protocol versions: initialization requirements, advertised tool surface without remote approval, repeat initialization rejection, unknown methods, ping and incompatible protocol metadata. Separate checks confirmed malformed-JSON recovery, rejection above the 1 MiB request limit and rejection of a non-private credential file.

## Bugs found and fixed

1. A commit filename after `--`, such as `-mfilename`, was interpreted as another message option. Preview parsing now respects the separator and makes explicit path selection unknown instead of claiming the complete staged index. A real fixture commit verifies the actual message.
2. PR options after `--` were shown as if GitHub CLI would parse them as flags. The structured parser now stops at the separator.
3. Trailing branch flags such as `--no-track` were shown as a branch start point. Unsupported option combinations now show unavailable structured details instead of inventing a start point.
4. Late cancellation changed already failed or outcome-unknown requests to cancelled. Native and MCP cancellation now preserve terminal execution status and audit history.

The original command string remains unchanged. These fixes improve explanation and result integrity; they do not restrict approved command combinations.

## Confirmed limits

- A timeout terminates the shell process group. A deliberately detached descendant can escape and continue; a disposable setsid probe reproduced this. Ordinary same-group delayed descendants were stopped. The probe completed in its disposable directory and was removed.
- Background jobs may finish after their parent shell exits. Exit success is not independent proof of all effects. The timeout result now explicitly warns about completed changes and potentially continuing background work.
- Static previews intentionally fall back to the exact command for runtime expansion or uncertain context. The tests do not establish a complete semantic understanding of arbitrary shell programs.
- The integration test uses real local Git commits and a bare remote. The gh process is an offline stand-in that records arguments and returns controlled success/failure; no actual GitHub pull request or authentication flow was exercised. A printed URL is not treated as a verified PR URL.
- Approval remains whole-command, with native approval simulated only inside isolated test brokers. This pass did not assess native dialog usability or foreground behavior.

## Eight additional cycles

Each of the three independent agents chose new cases for eight numbered cycles, ran each cycle separately, fixed confirmed defects, and reran its cycles. A fourth lane exercised the packaged MCP executable. This is eight cycles across four lanes, not a claim of exhaustive arbitrary-command verification.

| Cycle | Execution | Structured preview | Permission lifecycle | Packaged MCP wire |
| --- | --- | --- | --- | --- |
| 1 | Signal termination and exit semantics | Quoted/newline/dash/apostrophe filenames vs actual index | Only review triggers the prompt; duplicate review; auto-open off | Three supported protocol versions and public tool surface |
| 2 | Child closes stdin during a large write | Update mode excludes untracked, includes tracked deletions | Revoke/disable pending approval | Initialization guard and out-of-order notification |
| 3 | Invalid shell syntax prevents earlier effects | Directory selection excludes ignored and sibling files | Client-specific idempotency and recorded identity | Malformed JSON and unsupported root recovery |
| 4 | Binary output byte counts and digests | Direct argv message has no shell expansion | Denial reason boundaries and finality | Unicode/numeric response identity and unknown methods |
| 5 | Noninteractive Git/GH child settings | UTF-8 commit-message file and cleanup distinction | Queued cancel/revoke/disable: bug fixed, retested | Duplicate initialization and notifications |
| 6 | Working-directory changes remain local to execution | Offline GH attached flags, file body, owner head, empty body | Concurrent waiters receive the same denial | Version negotiation and incompatible metadata |
| 7 | Missing executable binding prevents execution | Unsupported staging modes stay explicitly uncertain | Queued approval expiry prevents command start | Exact UTF-8 byte limit and oversized rejection |
| 8 | Git shell aliases preserve arguments/configuration | Restaging and partial-stage oracles: bug fixed, retested | Current source permissions govern existing plans | Private-file, symlink and corrupt credential rejection |

All cells passed after fixes. Added **26 Rust regressions**: eight executor, eight lifecycle, and ten preview tests (cycle 8 has three cases). The complete suite now passes **138 Rust tests and 44 frontend tests**.

### Newly found and fixed

- **Restaging cancellation:** `git add file && git commit` could still list a staged file whose worktree had returned to HEAD, even though actual staging removed its index change. Overlapping restaging now marks the prospective commit file scope unknown and removes unverifiable diff membership. Nonoverlapping selections retain unrelated staged files with their correct Index/Worktree sources. This conservative correction does not read new blobs or run Git filters to simulate a future index.
- **Queued cancellation overwrite:** a worker waiting for the operation lock could overwrite a user's recorded cancellation with `needs_review`. Its error handler now preserves terminal outcomes. Tests cover native/MCP cancellation, credential revocation, connector disable and queued expiry; no command starts in these cases.

Reproduce wire cycles with `node scripts/test-mcp-wire.mjs` after building the packaged application, or supply an executable path as its first argument. Rust cycles live in the executor, preview and gate `blind_*` test modules. The offline GitHub and detached-process limitations from the initial pass still apply. No live user request was approved or actual GitHub write performed in these additional cycles.
