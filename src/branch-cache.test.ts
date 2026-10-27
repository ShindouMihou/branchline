import {it,expect,vi} from 'vitest';
import {createBranchCache,menuPosition} from './branch-cache';
it('coalesces prefetch, retains snapshots during refresh and isolates repositories',async()=>{
  const finish=new Map<string,(value:string[])=>void>();const load=vi.fn((root:string)=>new Promise<string[]>(resolve=>finish.set(root,resolve)));const cache=createBranchCache(load);
  const first=cache.refresh('/a');expect(cache.refresh('/a')).toBe(first);expect(load).toHaveBeenCalledTimes(1);
  finish.get('/a')!(['main']);await first;cache.invalidate('/a');expect(cache.peek('/a')).toEqual(['main']);const next=cache.refresh('/a');expect(cache.peek('/a')).toEqual(['main']);
  const other=cache.refresh('/b');finish.get('/b')!(['other']);await other;finish.get('/a')!(['main','feature']);await next;expect(cache.peek('/a')).toEqual(['main','feature']);expect(cache.peek('/b')).toEqual(['other']);
});
it('discards a stale in-flight result after invalidation and retries failed reads',async()=>{
  const resolvers:((value:string[])=>void)[]=[];const cache=createBranchCache(()=>new Promise<string[]>(resolve=>resolvers.push(resolve)));
  const old=cache.refresh('/repo');cache.invalidate('/repo');const fresh=cache.refresh('/repo');resolvers[1](['fresh']);await fresh;resolvers[0](['stale']);await old;expect(cache.peek('/repo')).toEqual(['fresh']);
  const load=vi.fn().mockRejectedValueOnce(Error('busy')).mockResolvedValueOnce(['main']);const retry=createBranchCache<string[]>(load);await expect(retry.refresh('/repo')).rejects.toThrow('busy');expect(retry.loading('/repo')).toBe(false);await retry.refresh('/repo');expect(retry.peek('/repo')).toEqual(['main']);
});
it('anchors a branch menu to its row and keeps it within the viewport',()=>{
  expect(menuPosition({x:600,y:200,alignRight:true},{width:240,height:200},{width:800,height:600})).toEqual({left:360,top:200});
  expect(menuPosition({x:790,y:590},{width:240,height:200},{width:800,height:600})).toEqual({left:552,top:392});
});
