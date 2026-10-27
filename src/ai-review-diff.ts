import {escapeHtml as e} from './html';
import {captureAiViewport,restoreAiViewport} from './ai-viewport';
import {parseDiff} from './diff';
import {fileLabel} from './file-label';
import {renderDiffHunks} from './diff-renderer';
import {applySourceTokens} from './source-tokens';
import {detectLanguage,languages,tokenize,type Tokens} from './syntax';
import type {AiDiffSelection,AiReviewDiff} from './types/ai-review-diff';

const icon=(name:string)=>`<i data-lucide="${name}" aria-hidden="true"></i>`;
export function createAiDiffReview(load:(request:object)=>Promise<AiReviewDiff>,repaint:()=>void){
  let selection:AiDiffSelection|null=null,result:AiReviewDiff|null=null,error='',loading=false,generation=0;
  let split=true,tokens:Tokens|null=null,syntaxGeneration=0;
  const overrides=new Map<string,string>();
  let observer:ResizeObserver|null=null;
  const language=()=>selection?(overrides.get(selection.path)??detectLanguage(selection.path)??'text'):'text';
  const paintTokens=()=>{const root=document.querySelector('.ai-review-diff-code');if(root&&tokens)applySourceTokens(root,tokens);};
  const highlight=async()=>{const active=result,id=++syntaxGeneration,lang=language();tokens=null;if(!active||lang==='text'||active.before===undefined&&active.after===undefined)return;try{const next=await tokenize({before:active.before??null,after:active.after??null},lang);if(id===syntaxGeneration&&active===result){tokens=next;paintTokens();}}catch{} };
  let originViewport:ReturnType<typeof captureAiViewport>=null,originId='';
  const close=(restore=true)=>{
    generation++;syntaxGeneration++;observer?.disconnect();selection=null;result=null;tokens=null;error='';loading=false;repaint();
    if(restore){restoreAiViewport(document,originViewport);document.getElementById(originId)?.focus({preventScroll:true});}
  };
  async function select(path:string){
    if(!selection)return;
    const inFiles=(document.activeElement as HTMLElement|null)?.hasAttribute('data-ai-diff-file');
    selection={...selection,path};const active=selection,id=++generation;
    loading=true;error='';result=null;tokens=null;syntaxGeneration++;repaint();
    if(inFiles){[...document.querySelectorAll<HTMLButtonElement>('[data-ai-diff-file]')].find(button=>button.dataset.aiDiffFile===path)?.focus({preventScroll:true});}else document.getElementById('ai-diff-back')?.focus({preventScroll:true});
    try{
      const next=await load({op:'review_diff',plan_id:active.planId,detail_index:active.detailIndex,path});
      if(id!==generation)return;result=next;
    }catch(reason){if(id===generation)error=String(reason);}
    finally{if(id===generation){loading=false;repaint();void highlight();}}
  }
  return {
    key:()=>selection?`diff:${selection.detailIndex}:${selection.path}`:'request',
    isOpen:()=>!!selection,
    reset:()=>{generation++;syntaxGeneration++;observer?.disconnect();selection=null;result=null;tokens=null;error='';loading=false;},
    open:(value:AiDiffSelection,origin:string)=>{
      originViewport=captureAiViewport(document);originId=origin;selection=value;void select(value.path);
    },
    close,
    render:()=>{
      if(!selection)return '';
      const scope=result?.source??selection.detail.filePreviews?.find(file=>file.path===selection?.path)?.source;
      const label=scope==='index'?'Currently staged':scope==='worktree'?selection.detail.kind==='index.stage'?'Changes to stage':'Projected commit':'Current file snapshot';
      const singleSide=result?.before===null&&typeof result.after==='string'?'added':result?.after===null&&typeof result.before==='string'?'deleted':null;
      const sideBySide=split&&!singleSide;
      const parsed=parseDiff(result?.patch??''),from=result?.baseline==='index'||selection.detail.kind==='index.stage'?'Index':'HEAD',to=scope==='index'?'Staged version':'Working tree';
      return `<div class="ai-diff-inspection"><header class="ai-diff-inspection-header"><button id="ai-diff-back" class="icon-button" aria-label="Back to request" title="Back to request (Escape)">${icon('chevron-left')}</button><div><strong>${e(selection.detail.title)}</strong><small>${e(selection.requestTitle)}</small></div></header><div class="ai-diff-inspection-body"><nav class="ai-review-diff-files" data-ai-scroll="files" aria-label="Files to review"><div>Changed files <span>${selection.detail.files?.length??0}</span></div>${selection.detail.files?.map((path,index)=>{return `<div class="file-row ${path===selection?.path?'selected':''}"><button class="file-select" id="ai-diff-file-${index}" data-ai-diff-file="${e(path)}" title="${e(path)}" aria-current="${path===selection?.path?'true':'false'}">${icon('file-code-2')}${fileLabel(path)}</button></div>`}).join('')}</nav><section class="ai-review-diff-panel" aria-label="Changes in ${e(selection.path)}"><div class="editor-tab ai-review-editor-tab">${icon('file-code-2')}<span title="${e(selection.path)}">${e(selection.path.split('/').at(-1))}</span><small>${e(label)}</small>${result?`<div class="ai-diff-totals"><span>${icon('plus')}${result.added}</span><span>${icon('minus')}${result.removed}</span></div>`:''}</div><div class="diff-toolbar revision-toolbar ${sideBySide?'split-head':''} ${singleSide?'single-version':''} ai-review-revisions"><span class="revision-from">${singleSide==='added'?'New file':singleSide==='deleted'?'Deleted file':from}</span><div class="revision-to">${singleSide?'':`<span>${to}</span>`}<div class="diff-controls"><select id="ai-diff-language" class="syntax-language" aria-label="Syntax highlighting language"><option value="auto" ${!overrides.has(selection.path)?'selected':''}>Auto${detectLanguage(selection.path)?`: ${languages.find(item=>item.id===detectLanguage(selection!.path))?.name??detectLanguage(selection.path)}`:''}</option><option value="text" ${overrides.get(selection.path)==='text'?'selected':''}>Plain text</option>${languages.map(item=>`<option value="${e(item.id)}" ${overrides.get(selection!.path)===item.id?'selected':''}>${e(item.name)}</option>`).join('')}</select><span class="diff-count">${parsed.hunks.length} ${parsed.hunks.length===1?'hunk':'hunks'}</span>${singleSide?'':`<button id="ai-diff-layout" class="quiet-control" aria-label="${split?'Show unified diff':'Show side-by-side diff'}" title="${split?'Show a unified diff':'Show changes side by side'}">${icon(split?'align-left':'columns-2')}${split?'Unified':'Side by side'}</button>`}<details class="ai-diff-snapshot-note" data-ai-disclosure="snapshot-details"><summary aria-label="About this snapshot" title="About this snapshot">${icon('info')}</summary><p>${e(result?.notice??'Read-only current snapshot. Earlier commands or later edits may change the final result.')}</p></details></div></div></div><div class="ai-review-diff-code ${sideBySide?'split-view':''}" data-ai-scroll="patch" tabindex="0" role="region" aria-busy="${loading}" aria-label="File diff">${loading?'<div class="ai-diff-feedback" role="status">Loading changes…</div>':error?`<div class="ai-diff-feedback" role="alert">${icon('circle-help')}<p>${e(error)}</p><button id="ai-diff-retry" class="ai-secondary-button">Try again</button></div>`:parsed.hunks.length?`<div class="ai-review-diff-table">${renderDiffHunks(parsed.hunks,sideBySide)}</div>`:`<div class="ai-diff-feedback"><p>No text changes are available for this snapshot.</p>${result?.notice?`<small>${e(result.notice)}</small>`:''}</div>`}</div>${sideBySide&&parsed.hunks.length?`<div class="diff-x-scrollbars" aria-label="Horizontal diff scrolling"><div data-ai-diff-scroll="before" data-ai-scroll="before" tabindex="0" aria-label="Scroll previous version horizontally"><div></div></div><div data-ai-diff-scroll="after" data-ai-scroll="after" tabindex="0" aria-label="Scroll new version horizontally"><div></div></div></div>`:''}</section></div></div>`;
    },
    bind:()=>{
      observer?.disconnect();
      const viewport=document.querySelector<HTMLElement>('.ai-review-diff-code'),tracks=[...document.querySelectorAll<HTMLElement>('[data-ai-diff-scroll]')];
      const resize=()=>{if(!viewport)return;viewport.style.setProperty('--diff-viewport',`${viewport.clientWidth}px`);tracks.forEach((track,index)=>{let width=track.clientWidth;viewport.querySelectorAll<HTMLElement>(`.code-cell:nth-child(${index+1}) code`).forEach(code=>width=Math.max(width,code.scrollWidth+46));if(track.firstElementChild)(track.firstElementChild as HTMLElement).style.width=`${width}px`;});};
      tracks.forEach((track,index)=>track.onscroll=()=>viewport?.style.setProperty(index?'--after-scroll':'--before-scroll',`${track.scrollLeft}px`));
      if(viewport&&typeof ResizeObserver!=='undefined'){observer=new ResizeObserver(resize);observer.observe(viewport);}resize();paintTokens();
      document.getElementById('ai-diff-layout')?.addEventListener('click',()=>{split=!split;repaint();});
      document.getElementById('ai-diff-language')?.addEventListener('change',event=>{if(!selection)return;const value=(event.target as HTMLSelectElement).value;if(value==='auto')overrides.delete(selection.path);else overrides.set(selection.path,value);void highlight();repaint();});
      document.getElementById('ai-diff-back')?.addEventListener('click',()=>close());
      document.getElementById('ai-diff-retry')?.addEventListener('click',()=>{if(selection)void select(selection.path);});
      document.querySelectorAll<HTMLButtonElement>('[data-ai-diff-file]').forEach(button=>button.onclick=()=>void select(button.dataset.aiDiffFile!));
    },
  };
}
