# Approval review: UX assessment

27 October 2026. Expert cognitive walkthrough of the implemented UI and a local fixture workflow. This is not a study with participants; no comprehension scores or user success rates have been measured.

## Main finding

The remaining weakness was interaction and information order. Operation colors made the view more distinct, but users still could not inspect the actual content they were being asked to permit. A permission review needs an inspection path, a clear return path, and an unmistakable scope of approval.

## Tasks and findings

| Priority | Task | Observed gap | Implemented response |
|---|---|---|---|
| High | Inspect the files being staged or committed | File names were inert text, so review stopped at the path. | Card-level Review changes action and clickable full file rows open a file-by-file diff inside the request. |
| High | Understand what the preview establishes | Current/prospective labels were inferred from prose, and a future staged selection could resemble a confirmed commit. | Typed evidence and per-file index/worktree sources. Diff header identifies Currently staged, Changes to stage, or Projected commit; an accessible info disclosure explains the snapshot scope. |
| Medium | Recognize a queued request | Several inbox entries had the same Review 1 command title. | Inbox uses the same human-readable request summary as the main heading. |
| Medium | Scan the planned work | The long raw command led the review and pushed useful action details downward. | Compact linked action overview; exact unchanged command remains available in an explicit disclosure, with the structured cards immediately inspectable. |
| Medium | Return from inspecting a file | Replacing the modal DOM during polling could lose scroll, file selection, and focus. | Inspection stays in the existing request; Back to request/Escape restores origin position and button focus. File/diff scroll is preserved across ordinary redraws. |
| Medium | Know what Approve authorizes | Inspecting a single file could imply a file-level permission decision. | Fixed decision footer explicitly says Approval covers the entire command. The same Approve/Deny/Deny with a reason actions remain. |

## Inspection contract

Selecting a file performs a native-only, bounded, read-only comparison. It does not stage files, execute the proposed command, approve anything, contact a remote, or disclose the loaded diff through MCP. Stage compares the current index with the working file. Commit compares HEAD with the current index, or with working content for files an earlier staging action selects; unrelated already-staged files retain their index source. These comparisons are distinct for partially staged files.

The loaded comparison is a snapshot taken on click, not a guarantee of the final command result. Earlier commands, filters, hooks, edits, modes, and rename semantics can affect that result. Binary/oversized/inaccessible content receives an explicit notice or inline error. Unknown dynamic selections remain unknown rather than becoming an invented diff.

## Remaining validation

- Test with people who use Git routinely but do not know its plumbing: identify the commit content, PR destination, and whole-request approval scope without assistance.
- Matching Stage/Commit file sets now appear only under Commit, while both card headings retain their distinct diff entry points. Test whether the compact Stage explanation makes this relationship clear; different file sets remain separate.
- Evaluate very large requests and narrow windows for navigation effort; file previews are intentionally bounded.
- Check screen-reader comprehension of current versus projected scope and keyboard navigation between the request, file list, diff, and decisions.

These are validation questions, not established user findings. The code and fixture walkthrough establish functionality and navigation behavior, not a measured usability rating.

### Main diff visual consistency

Approval cards and file inspection now reuse the main diff view's icon language, filename/directory hierarchy, source rendering and syntax colors. Read-only inspection offers the same split/unified choice and language picker, with current-snapshot context available through the info icon. Operation cards remain distinct, and identical staged/commit file lists remain consolidated under Commit.
