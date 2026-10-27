// @vitest-environment happy-dom
import {beforeEach,expect,it,vi} from 'vitest';
import {createStashes} from './stashes';
import type {StashHooks} from './types/stashes';
vi.mock('./syntax',()=>({detectLanguage:()=>null,tokenize:vi.fn()}));
const entry={reference:'stash@{0}',oid:'a'.repeat(40),message:'Paused work',branch:'feature/help',timestamp:1793120000};
const patch='diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+saved\n';
beforeEach(()=>{document.body.innerHTML='';});
function setup(overrides:Partial<StashHooks>={}){
 const requests:Record<string,unknown>[]=[];
 const api=vi.fn(async(request:object)=>{
  const data=(request as {request:Record<string,unknown>}).request;requests.push(data);
  switch(data.action){
   case 'list':return [entry];
   case 'files':return {files:[{path:'a.txt',source:'tracked'},{path:'b.txt',source:'tracked'}],baseline:'Original HEAD'};
   case 'diff':return {patch,before:'old\n',after:'saved\n'};
   case 'preview':return {token:'fresh',files:[{path:'a.txt',index:'M',worktree:'M'}],scope:data.scope};
   case 'restore_preview':return {token:'restore-token',dirty:true};
   case 'restore':return {ok:false,outcome:'restore_failed',message:'Conflict. The stash was kept.',conflicts:['a.txt'],recoveryRef:'refs/branchline-backups/test'};
   case 'save':return {ok:true,outcome:'saved',message:'Saved.'};
   case 'drop':return {ok:true,outcome:'deleted',message:'Removed.'};
   default:throw Error('Unexpected request');
  }
 });
 let stashes:ReturnType<typeof createStashes>;
 const render=()=>{document.body.innerHTML=stashes.render()+stashes.renderDialog();stashes.bind();};
 const hooks:StashHooks={root:()=>'/repo',files:()=>[{path:'a.txt',index:'M',worktree:'M'},{path:'b.txt',index:' ',worktree:'M'}],render,api:api as StashHooks['api'],changed:vi.fn(async()=>{}),openChanges:vi.fn(),setBusy:vi.fn(),...overrides};
 stashes=createStashes(hooks);render();
 return {stashes,render,requests,hooks};
}
it('browses files without resetting the list scroll and escapes names',async()=>{
 const {stashes}=setup();await stashes.refresh();
 expect(document.querySelector('.stash-diff-code')?.textContent).toContain('saved');
 const list=document.querySelector<HTMLElement>('[data-stash-scroll="files"]')!;list.scrollTop=250;
 document.querySelector<HTMLButtonElement>('[data-stash-file="1"]')!.click();
 await vi.waitFor(()=>expect(document.querySelector('.stash-diff-panel .editor-tab')?.textContent).toContain('b.txt'));
 expect(document.querySelector('[data-stash-scroll="files"]')!.scrollTop).toBe(250);
});
it('previews unstaged-only saving, sends the freshness token, and locks other operations',async()=>{
 const {stashes,requests,hooks}=setup();await stashes.refresh();stashes.showSave();
 await vi.waitFor(()=>expect((document.getElementById('stash-dialog-submit') as HTMLButtonElement).disabled).toBe(false));
 const scope=document.getElementById('stash-scope') as HTMLSelectElement;scope.value='unstaged';scope.dispatchEvent(new Event('change'));
 await vi.waitFor(()=>expect(requests.at(-1)?.scope).toBe('unstaged'));
 await vi.waitFor(()=>expect((document.getElementById('stash-dialog-submit') as HTMLButtonElement).disabled).toBe(false));
 document.getElementById('stash-dialog-submit')!.click();
 await vi.waitFor(()=>expect(requests.some(request=>request.action==='save')).toBe(true));
 expect(requests.find(request=>request.action==='save')).toMatchObject({scope:'unstaged',expected:'fresh',expected_root:'/repo',include_untracked:false});
 expect(hooks.setBusy).toHaveBeenCalledWith(true);await vi.waitFor(()=>expect(hooks.setBusy).toHaveBeenLastCalledWith(false));
});
it('explains Pop, sends the index choice, and keeps a failed restore visible',async()=>{
 const {stashes,requests}=setup();await stashes.refresh();document.getElementById('stash-pop')!.click();
 await vi.waitFor(()=>expect((document.getElementById('stash-dialog-submit') as HTMLButtonElement).disabled).toBe(false));
 expect(document.querySelector('.stash-modal')?.textContent).toContain('If there’s a conflict, the stash stays.');
 const index=document.getElementById('stash-restore-index') as HTMLInputElement;index.checked=true;index.dispatchEvent(new Event('change'));
 document.getElementById('stash-dialog-submit')!.click();
 await vi.waitFor(()=>expect(document.querySelector('.stash-feedback')?.textContent).toContain('The stash was kept'));
 expect(requests.find(request=>request.action==='restore')).toMatchObject({pop:true,restore_index:true,expected:'restore-token',oid:entry.oid});
 expect(document.querySelectorAll('.stash-entry')).toHaveLength(1);expect(document.getElementById('stash-conflicts')).not.toBeNull();
});
it('requires confirmation before deleting and separates Apply from Pop',async()=>{
 const {stashes,requests}=setup();await stashes.refresh();document.getElementById('stash-delete')!.click();
 expect(requests.some(request=>request.action==='drop')).toBe(false);
 document.getElementById('stash-dialog-cancel')!.click();document.getElementById('stash-apply')!.click();
 await vi.waitFor(()=>expect(document.querySelector('.stash-modal')?.textContent).toContain('keep the stash for later'));
});
it('discards an old repository response after switching repositories',async()=>{
 let resolve:(value:unknown)=>void=()=>{};let root='/old';
 const pending=new Promise<unknown>(done=>{resolve=done;});
 const {stashes}=setup({root:()=>root,api:(()=>pending) as StashHooks['api']});
 const loading=stashes.refresh();root='/new';stashes.reset();resolve([entry]);await loading;
 expect(document.querySelectorAll('.stash-entry')).toHaveLength(0);
});
