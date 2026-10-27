// @vitest-environment happy-dom
import {expect,it} from 'vitest';
import {parseDiff} from './diff';
import {renderDiffHunks} from './diff-renderer';
import {applySourceTokens} from './source-tokens';
const hunks=parseDiff('@@ -8,2 +8,2 @@\n-<old>\n+<new>\n context\n').hunks;
it('shares escaped, read-only split and unified source rows with accurate line numbers',()=>{
 for(const split of [true,false]){
  document.body.innerHTML=renderDiffHunks(hunks,split);
  expect(document.querySelector('old')).toBeNull();expect(document.querySelector('new')).toBeNull();
  expect(document.querySelector('[data-syntax-side="before"][data-syntax-line="8"]')?.textContent).toBe('<old>');
  expect(document.querySelector('[data-syntax-side="after"][data-syntax-line="8"]')?.textContent).toBe('<new>');
  expect(document.querySelector('button')).toBeNull();
 }
});
it('keeps mutation controls explicit for the main viewer',()=>{
 document.body.innerHTML=renderDiffHunks(hunks,true,index=>`<button data-hunk="${index}">Stage hunk</button>`);
 expect(document.querySelector('[data-hunk="0"]')?.textContent).toBe('Stage hunk');
});
it('applies tokens only to matching snapshot text',()=>{
 document.body.innerHTML=renderDiffHunks(hunks,true);
 applySourceTokens(document,{before:Array.from({length:8},(_,i)=>({text:i===7?'<old>':'',html:'<span data-token-color="#aabbcc">&lt;old&gt;</span>'})),after:Array.from({length:8},()=>({text:'stale',html:'wrong'}))});
 expect(document.querySelector('[data-syntax-side="before"][data-syntax-line="8"] span')?.textContent).toBe('<old>');
 expect(document.querySelector('[data-syntax-side="after"][data-syntax-line="8"]')?.textContent).toBe('<new>');
});
