// @vitest-environment happy-dom
import {it,expect,vi} from 'vitest';
import {createAiDiffReview} from './ai-review-diff';
import {captureAiViewport,restoreAiViewport} from './ai-viewport';
import type {AiReviewDiff} from './types/ai-review-diff';
const detail={commandIndex:0,kind:'commit.create',title:'Commit changes',files:['a.ts','b.ts'],filePreviews:[{path:'a.ts',source:'index' as const},{path:'b.ts',source:'worktree' as const}],warnings:[],evidence:'Snapshot',evidenceKind:'current_snapshot' as const};
const response=(path:string):AiReviewDiff=>({path,source:path==='a.ts'?'index':'worktree',patch:'diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1,2 @@\n-old <tag>\n+new <tag>\n+++ actual content\n',snapshotId:'snapshot',added:1,removed:1,notice:'Current snapshot only.'});
it('opens a read-only file inspection, switches files, and returns to the originating request position',async()=>{
 const load=vi.fn(async(request:object)=>response((request as {path:string}).path));
 let viewer:ReturnType<typeof createAiDiffReview>;
 const render=()=>{const viewport=captureAiViewport(document);document.body.innerHTML=`<section class="ai-modal" data-ai-view="${viewer.key()}">${viewer.isOpen()?viewer.render():'<div class="ai-review-scroll"><button id="origin">Review changes</button></div>'}</section>`;viewer.bind();restoreAiViewport(document,viewport);};
 viewer=createAiDiffReview(load,render);render();document.querySelector('.ai-review-scroll')!.scrollTop=320;document.getElementById('origin')!.focus();
 viewer.open({planId:'request-a',detailIndex:2,detail,path:'a.ts',requestTitle:'Save welcome'},'origin');
 await vi.waitFor(()=>expect(document.querySelector('.ai-review-diff-code')?.textContent).toContain('new <tag>'));
 expect(load).toHaveBeenCalledWith({op:'review_diff',plan_id:'request-a',detail_index:2,path:'a.ts'});
 expect(document.querySelector('tag')).toBeNull();expect(document.querySelector('.ai-review-diff-code')?.textContent).toContain('++ actual content');expect(document.body.textContent).toContain('Currently staged');
 expect(document.querySelector('.split-lines')).not.toBeNull();expect(document.querySelector('[data-hunk]')).toBeNull();
 document.getElementById('ai-diff-layout')!.click();expect(document.querySelector('.unified-lines')).not.toBeNull();expect(document.querySelector('.ai-review-diff-code')?.textContent).toContain('++ actual content');
 expect(document.getElementById('ai-diff-back')?.getAttribute('aria-label')).toBe('Back to request');
 expect(document.getElementById('ai-diff-back')?.textContent?.trim()).toBe('');
 expect((document.querySelector('.ai-diff-snapshot-note') as HTMLDetailsElement).open).toBe(false);
 (document.querySelector('[data-ai-diff-file="b.ts"]') as HTMLButtonElement).click();
 await vi.waitFor(()=>expect(document.body.textContent).toContain('Projected commit'));
 document.querySelector('.ai-review-diff-code')!.scrollTop=140;document.querySelector('.ai-review-diff-code')!.scrollLeft=90;render();
 expect(document.querySelector('.ai-review-diff-code')!.scrollTop).toBe(140);expect(document.querySelector('.ai-review-diff-code')!.scrollLeft).toBe(90);
 document.getElementById('ai-diff-back')!.click();
 expect(viewer.isOpen()).toBe(false);expect(document.querySelector('.ai-review-scroll')!.scrollTop).toBe(320);expect(document.activeElement?.id).toBe('origin');
});
it('handles errors inline and ignores a response arriving after the inspection closes',async()=>{
 let finish:(value:AiReviewDiff)=>void=()=>{};
 const load=vi.fn().mockRejectedValueOnce(new Error('File changed')).mockImplementationOnce(()=>new Promise<AiReviewDiff>(resolve=>{finish=resolve}));
 let viewer:ReturnType<typeof createAiDiffReview>;const render=()=>{document.body.innerHTML=viewer.render();viewer.bind()};viewer=createAiDiffReview(load,render);
 viewer.open({planId:'p',detailIndex:0,detail,path:'a.ts',requestTitle:'Review'},'');
 await vi.waitFor(()=>expect(document.querySelector('[role=alert]')?.textContent).toContain('File changed'));
 document.getElementById('ai-diff-retry')!.click();viewer.close();finish(response('a.ts'));
 await Promise.resolve();await Promise.resolve();expect(viewer.isOpen()).toBe(false);expect(document.body.textContent).toBe('');
});
it('shows an explicit empty state for a metadata-only patch',async()=>{
 let viewer:ReturnType<typeof createAiDiffReview>;const render=()=>{document.body.innerHTML=viewer.render();viewer.bind()};
 viewer=createAiDiffReview(async()=>({...response('a.ts'),patch:'diff --git a/a.ts b/a.ts\nnew file mode 100644\n'}),render);
 viewer.open({planId:'p',detailIndex:0,detail,path:'a.ts',requestTitle:'Review'},'');
 await vi.waitFor(()=>expect(document.querySelector('.ai-diff-feedback')?.textContent).toContain('No text changes'));
 viewer.reset();
});
it('keeps file icons stable when added-file snapshots finish loading and files are selected',async()=>{
 let viewer:ReturnType<typeof createAiDiffReview>;
 const render=()=>{document.body.innerHTML=viewer.render();viewer.bind()};
 const added=(path:string)=>({...response(path),before:null,after:'new <tag>\n++ actual content\n'});
 viewer=createAiDiffReview(async(request:object)=>added((request as {path:string}).path),render);
 viewer.open({planId:'p',detailIndex:0,detail,path:'a.ts',requestTitle:'Review'},'');
 const icons=()=>[...document.querySelectorAll('[data-ai-diff-file]')].map(button=>button.querySelector('[data-lucide]')?.getAttribute('data-lucide'));
 expect(icons()).toEqual(['file-code-2','file-code-2']);
 await vi.waitFor(()=>expect(document.querySelector('.ai-review-diff-code')?.getAttribute('aria-busy')).toBe('false'));
 expect(icons()).toEqual(['file-code-2','file-code-2']);
 (document.querySelector('[data-ai-diff-file="b.ts"]') as HTMLButtonElement).click();
 await vi.waitFor(()=>expect(document.querySelector('.ai-review-diff-code')?.getAttribute('aria-busy')).toBe('false'));
 expect(icons()).toEqual(['file-code-2','file-code-2']);
 expect(document.querySelector('.ai-review-diff-files .file-status')).toBeNull();
 viewer.reset();
});
it('uses the full width for added and deleted files without changing the split preference for modified files',async()=>{
 let viewer:ReturnType<typeof createAiDiffReview>;
 const render=()=>{document.body.innerHTML=viewer.render();viewer.bind()};
 let snapshot:AiReviewDiff={...response('a.ts'),before:null,after:'new <tag>\n++ actual content\n'};
 viewer=createAiDiffReview(async()=>snapshot,render);
 for(const kind of ['added','deleted','modified']){
  snapshot={...response('a.ts'),before:kind==='added'?null:'old <tag>\n',after:kind==='deleted'?null:'new <tag>\n++ actual content\n'};
  viewer.open({planId:'p',detailIndex:0,detail,path:'a.ts',requestTitle:'Review'},'');
  await vi.waitFor(()=>expect(document.querySelector('.ai-review-diff-code')?.getAttribute('aria-busy')).toBe('false'));
  expect(!!document.querySelector('.split-lines')).toBe(kind==='modified');
  expect(!!document.getElementById('ai-diff-layout')).toBe(kind==='modified');
  if(kind!=='modified'){
   expect(document.querySelector('.unified-lines')).not.toBeNull();
   expect(document.querySelector('.revision-from')?.textContent).toBe(kind==='added'?'New file':'Deleted file');
   expect(document.querySelector('.diff-x-scrollbars')).toBeNull();
  }
  viewer.reset();
 }
});
