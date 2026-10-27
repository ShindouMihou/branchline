import { it, expect, vi } from 'vitest';
import { createAutoRefresh, type Revision } from './auto-refresh';
it('coalesces changes, defers approval/interaction, and audits missed events',async()=>{
  let current:Revision={root:'/repo',revision:'1',watching:true};let revision='1',allowed=true,now=1000;
  const refresh=vi.fn(async()=>{current={...current,revision};});
  const tick=createAutoRefresh({current:()=>current,allowed:()=>allowed,revision:async()=>({...current,revision}),refresh,now:()=>now});
  await tick();expect(refresh).not.toHaveBeenCalled();
  revision='2';allowed=false;await tick();expect(refresh).not.toHaveBeenCalled();
  allowed=true;await tick();expect(refresh).toHaveBeenCalledTimes(1);
  await tick();expect(refresh).toHaveBeenCalledTimes(1);
  now=32000;await tick();expect(refresh).toHaveBeenCalledTimes(2);
});
it('ignores late signals from a previously opened repository',async()=>{
  let current:Revision={root:'/old',revision:'0'};let resolve!:(r:Revision)=>void;
  const refresh=vi.fn(async()=>{});
  const tick=createAutoRefresh({current:()=>current,allowed:()=>true,revision:()=>new Promise<Revision>(r=>{resolve=r;}),refresh});
  const pending=tick();await tick();current={root:'/new',revision:'0'};resolve({root:'/old',revision:'1',watching:true});await pending;
  expect(refresh).not.toHaveBeenCalled();
});

it('uses a frequent status fallback when native watching is unavailable',async()=>{
  const current:Revision={root:'/repo',revision:'0',watching:false};let now=4999;
  const refresh=vi.fn(async()=>{});
  const tick=createAutoRefresh({current:()=>current,allowed:()=>true,revision:async()=>current,refresh,now:()=>now});
  await tick();expect(refresh).not.toHaveBeenCalled();
  now=5000;await tick();expect(refresh).toHaveBeenCalledTimes(1);
});
