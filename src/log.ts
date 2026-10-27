export interface GraphCommit { oid: string; parents: string[] }
export interface GraphRow { lane: number; width: number; incoming: number[]; edges: { from: number; to: number; color: number }[]; color: number }
const color = (oid: string) => [...oid.slice(0, 8)].reduce((n, c) => n + c.charCodeAt(0), 0) % 4;
/** Follow parent identities through topo-ordered rows, including crossing branch lanes. */
export function graphRows(commits: GraphCommit[]): GraphRow[] {
  let lanes: string[] = [];
  return commits.map(commit => {
    let lane = lanes.indexOf(commit.oid);
    if (lane < 0) { lane = lanes.length; lanes.push(commit.oid); }
    const before = [...lanes];
    lanes.splice(lane, 1);
    commit.parents.forEach((parent, i) => { if (!lanes.includes(parent)) lanes.splice(Math.min(lane + i, lanes.length), 0, parent); });
    const edges: GraphRow['edges'] = [];
    before.forEach((oid, i) => {
      if (oid !== commit.oid) edges.push({ from: i, to: lanes.indexOf(oid), color: color(oid) });
      else commit.parents.forEach(parent => edges.push({ from: i, to: lanes.indexOf(parent), color: color(parent) }));
    });
    return { lane, width: Math.max(before.length, lanes.length), incoming: before.map((_, i) => i), edges, color: color(commit.oid) };
  });
}
