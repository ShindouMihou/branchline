export interface DiffLine { kind: 'context' | 'add' | 'remove' | 'meta'; text: string; old: number | null; new: number | null }
export interface Hunk { header: string; lines: DiffLine[]; index: number }
export function parseDiff(patch: string): { headers: string[]; hunks: Hunk[] } {
  const headers: string[] = [], hunks: Hunk[] = []; let old = 0, next = 0;
  for (const line of patch.split('\n')) {
    const match = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(line);
    if (match) { old = Number(match[1]); next = Number(match[2]); hunks.push({ header: line, index: hunks.length, lines: [] }); continue; }
    const hunk = hunks.at(-1);
    if (!hunk) { if (line) headers.push(line); continue; }
    if (line.startsWith('+')) hunk.lines.push({ kind: 'add', text: line.slice(1), old: null, new: next++ });
    else if (line.startsWith('-')) hunk.lines.push({ kind: 'remove', text: line.slice(1), old: old++, new: null });
    else if (line.startsWith(' ')) hunk.lines.push({ kind: 'context', text: line.slice(1), old: old++, new: next++ });
    else if (line.startsWith('\\')) hunk.lines.push({ kind: 'meta', text: line, old: null, new: null });
  }
  return { headers, hunks };
}
export function pairedLines(lines: DiffLine[]): [DiffLine | null, DiffLine | null][] {
  const rows: [DiffLine | null, DiffLine | null][] = []; let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.kind === 'context' || line.kind === 'meta') { rows.push([line, line]); i++; continue; }
    const removed: DiffLine[] = [], added: DiffLine[] = [];
    while (i < lines.length && lines[i].kind === 'remove') removed.push(lines[i++]);
    while (i < lines.length && lines[i].kind === 'add') added.push(lines[i++]);
    for (let j = 0; j < Math.max(removed.length, added.length); j++) rows.push([removed[j] ?? null, added[j] ?? null]);
  }
  return rows;
}
