export interface Revision { root: string; revision?: string; watching?: boolean }
interface Hooks {
  current: () => Revision | null;
  allowed: () => boolean;
  revision: () => Promise<Revision>;
  refresh: () => Promise<void>;
  now?: () => number;
}
/** Coalesce filesystem signals and defer them while the user interacts or approves a plan. */
export function createAutoRefresh(hooks: Hooks) {
  let inFlight=false, lastAudit=0;
  const now=hooks.now??Date.now;
  return async function tick() {
    if(inFlight||!hooks.current()||!hooks.allowed())return;
    inFlight=true;
    try {
      const before=hooks.current()!;
      const signal=await hooks.revision();
      if(!hooks.allowed()||hooks.current()?.root!==before.root||signal.root!==before.root)return;
      const auditDue=now()-lastAudit>=(signal.watching?30000:5000);
      if(signal.revision!==hooks.current()?.revision||auditDue){
        await hooks.refresh();lastAudit=now();
      }
    } finally { inFlight=false; }
  };
}
