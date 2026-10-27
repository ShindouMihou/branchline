import {stashView,stashDialog} from './stash-view';
import {bindSplitDiffScroll} from './diff-scroll';
import {detectLanguage,tokenize,type Tokens} from './syntax';
import {applySourceTokens} from './source-tokens';
import {copyText} from './clipboard';
import type {StashDialog,StashDiff,StashEntry,StashFile,StashHooks,StashImage,StashPreview,StashResult,StashScope,StashView} from './types/stashes';
import './stashes.css';

export function createStashes(hooks:StashHooks){
 const view:StashView={entries:[],entry:null,files:[],path:'',baseline:'Original HEAD',staged:false,filter:'',loading:false,loadingDiff:false,error:'',result:null,diff:null,split:true,image:null,imageMode:false,busy:false};
 let root='',generation=0,fileGeneration=0,previewGeneration=0,dialog:StashDialog|null=null,dialogError='',tokens:Tokens|null=null,observer:ResizeObserver|undefined;
 const api=<T>(request:object)=>hooks.api<T>({op:'stash',request:{...request,expected_root:root}});
 const repaint=()=>{
  const offsets=[...document.querySelectorAll<HTMLElement>('[data-stash-scroll]')].map(el=>[el.dataset.stashScroll!,el.scrollTop,el.scrollLeft] as const);
  hooks.render();
  for(const [name,top,left] of offsets){const el=document.querySelector<HTMLElement>(`[data-stash-scroll="${name}"]`);if(el){el.scrollTop=top;el.scrollLeft=left;}}
 };
 function reset(){generation++;fileGeneration++;previewGeneration++;root=hooks.root()??'';dialog=null;dialogError='';tokens=null;Object.assign(view,{entries:[],entry:null,files:[],path:'',diff:null,image:null,imageMode:false,error:'',result:null,loading:false,loadingDiff:false,staged:false,filter:''});}
 async function readFile(path:string){
  if(!view.entry)return;
  const id=++fileGeneration,oid=view.entry.oid,staged=view.staged;
  view.path=path;view.error='';view.loadingDiff=true;view.diff=null;view.image=null;view.imageMode=false;tokens=null;repaint();
  try{
   const diff=await api<StashDiff>({action:'diff',oid,path,staged});
   if(id!==fileGeneration)return;view.diff=diff;
   const language=detectLanguage(path);
   if(language)void tokenize(diff,language).then(result=>{if(id===fileGeneration){tokens=result;const el=document.querySelector('.stash-diff-code');if(el)applySourceTokens(el,result);}});
   if(/\.(svg|png|jpe?g|gif|webp|avif|bmp|ico)$/i.test(path)){
    const image=await api<StashImage>({action:'image',oid,path,staged});
    if(id!==fileGeneration)return;view.image=image.before||image.after?image:null;view.imageMode=!!view.image;
   }
  }catch(error){if(id===fileGeneration)view.error=String(error);}
  finally{if(id===fileGeneration){view.loadingDiff=false;repaint();}}
 }
 async function select(entry:StashEntry,staged=false){
  const id=++generation;fileGeneration++;view.entry=entry;view.staged=staged;view.files=[];view.path='';view.diff=null;view.image=null;view.loadingDiff=false;tokens=null;view.loading=true;view.error='';repaint();
  try{
   const result=await api<{files:StashFile[];baseline:string}>({action:'files',oid:entry.oid,staged});
   if(id!==generation)return;view.files=result.files;view.baseline=result.baseline;
   if(result.files.length)await readFile(result.files[0].path);
  }catch(error){if(id===generation)view.error=String(error);}
  finally{if(id===generation){view.loading=false;repaint();}}
 }
 async function refresh(){
  if(root!==hooks.root())reset();
  if(!root||view.busy||dialog)return;
  const id=++generation;view.loading=true;view.error='';
  try{
   const entries=await api<StashEntry[]>({action:'list'});if(id!==generation)return;
   const same=JSON.stringify(entries)===JSON.stringify(view.entries);view.entries=entries;
   const entry=entries.find(entry=>entry.oid===view.entry?.oid)??entries[0]??null;
   if(entry&&entry.oid!==view.entry?.oid){await select(entry);return;}
   if(entry)view.entry=entry;
   else{view.entry=null;view.files=[];view.path='';view.diff=null;view.image=null;view.loadingDiff=false;fileGeneration++;}
   if(!same)repaint();
  }catch(error){if(id===generation)view.error=String(error);}
  finally{if(id===generation){view.loading=false;repaint();}}
 }
 async function preview(){
  const current=dialog,id=++previewGeneration;if(!current||current.kind==='delete')return;
  current.preview=null;dialogError='';repaint();
  try{
   if(current.kind==='save'){
    const result=await api<StashPreview>({action:'preview',scope:current.scope,paths:[...current.paths],include_untracked:current.includeUntracked});
    if(dialog===current&&id===previewGeneration)current.preview=result;
   }else{
    const result=await api<{token:string;dirty:boolean}>({action:'restore_preview',reference:current.entry.reference,oid:current.entry.oid});
    if(dialog===current&&id===previewGeneration)current.preview=result;
   }
  }catch(error){if(dialog===current&&id===previewGeneration)dialogError=String(error);}
  finally{if(dialog===current&&id===previewGeneration)repaint();}
 }
 function showSave(){if(root!==hooks.root())reset();dialog={kind:'save',scope:'all',name:'',includeUntracked:false,paths:new Set(),preview:null};void preview();}
 function close(){if(view.busy)return;dialog=null;previewGeneration++;dialogError='';repaint();}
 async function submit(){
  const current=dialog;if(!current||view.busy||dialogError)return;
  if(current.kind!=='delete'&&!current.preview)return;
  view.busy=true;hooks.setBusy(true);dialogError='';repaint();
  try{
   const request=current.kind==='save'?{action:'save',scope:current.scope,paths:[...current.paths],include_untracked:current.includeUntracked,message:current.name,expected:current.preview!.token}:current.kind==='restore'?{action:'restore',reference:current.entry.reference,oid:current.entry.oid,pop:current.pop,restore_index:current.restoreIndex,expected:current.preview!.token}:{action:'drop',reference:current.entry.reference,oid:current.entry.oid};
   view.result=await api<StashResult>(request);dialog=null;
   try{await hooks.changed(view.result);}catch(error){view.result={...view.result,message:`${view.result.message} Repository status could not be refreshed: ${String(error)}`};}
  }catch(error){dialogError=String(error);if(current.kind!=='delete')current.preview=null;}
  finally{view.busy=false;hooks.setBusy(false);if(!dialog)await refresh();repaint();}
 }
 function bind(){
  observer?.disconnect();
  const on=(id:string,fn:()=>void)=>document.getElementById(id)?.addEventListener('click',fn);
  on('stash-new',showSave);on('stash-empty-new',showSave);on('stash-from-changes',showSave);
  on('stash-refresh',()=>void refresh());on('stash-retry',()=>void refresh());
  on('stash-apply',()=>{if(view.entry){dialog={kind:'restore',entry:view.entry,pop:false,restoreIndex:false,preview:null};void preview();}});
  on('stash-pop',()=>{if(view.entry){dialog={kind:'restore',entry:view.entry,pop:true,restoreIndex:false,preview:null};void preview();}});
  on('stash-delete',()=>{if(view.entry){dialogError='';dialog={kind:'delete',entry:view.entry};repaint();}});
  on('stash-dialog-close',close);on('stash-dialog-cancel',close);on('stash-dialog-submit',()=>void submit());
  on('stash-preview-retry',()=>void preview());
  on('stash-dismiss',()=>{view.result=null;repaint();});on('stash-conflicts',hooks.openChanges);
  on('stash-copy-recovery',()=>{if(view.result?.recoveryRef)void copyText(view.result.recoveryRef).catch(error=>{view.error=String(error);repaint();});});
  on('stash-layout',()=>{view.split=!view.split;repaint();});on('stash-image-mode',()=>{view.imageMode=!view.imageMode;repaint();});
  document.getElementById('stash-search')?.addEventListener('input',event=>{view.filter=(event.target as HTMLInputElement).value;repaint();});
  document.querySelectorAll<HTMLButtonElement>('[data-stash-entry]').forEach(button=>button.onclick=()=>{const entry=view.entries.find(entry=>entry.reference===button.dataset.stashEntry);if(entry)void select(entry);});
  document.querySelectorAll<HTMLButtonElement>('[data-stash-file]').forEach(button=>button.onclick=()=>void readFile(view.files[Number(button.dataset.stashFile)].path));
  document.querySelectorAll<HTMLButtonElement>('[data-stash-view]').forEach(button=>button.onclick=()=>{if(view.entry)void select(view.entry,button.dataset.stashView==='index');});
  document.getElementById('stash-name')?.addEventListener('input',event=>{if(dialog?.kind==='save')dialog.name=(event.target as HTMLInputElement).value;});
  document.getElementById('stash-scope')?.addEventListener('change',event=>{if(dialog?.kind==='save'){dialog.scope=(event.target as HTMLSelectElement).value as StashScope;void preview();}});
  document.getElementById('stash-untracked')?.addEventListener('change',event=>{if(dialog?.kind==='save'){dialog.includeUntracked=(event.target as HTMLInputElement).checked;void preview();}});
  document.getElementById('stash-restore-index')?.addEventListener('change',event=>{if(dialog?.kind==='restore')dialog.restoreIndex=(event.target as HTMLInputElement).checked;});
  document.querySelectorAll<HTMLInputElement>('[data-stash-pick]').forEach(input=>input.onchange=()=>{if(dialog?.kind==='save'){if(input.checked)dialog.paths.add(input.value);else dialog.paths.delete(input.value);void preview();}});
  const modal=document.querySelector<HTMLElement>('.stash-modal');
  modal?.addEventListener('keydown',event=>{
   if(event.key==='Escape'){event.preventDefault();close();}
   if(event.key==='Tab'){const controls=[...modal.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),summary')];if(!controls.length)return;const first=controls[0],last=controls.at(-1)!;if(event.shiftKey&&document.activeElement===first){event.preventDefault();last.focus();}else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first.focus();}}
  });
  if(modal&&!modal.contains(document.activeElement))modal.querySelector<HTMLElement>('input,button')?.focus({preventScroll:true});
  const viewport=document.querySelector<HTMLElement>('.stash-diff-code'),tracks=[...document.querySelectorAll<HTMLElement>('[data-stash-x]')];
  const size=()=>{if(!viewport)return;viewport.style.setProperty('--diff-viewport',`${viewport.clientWidth}px`);tracks.forEach((track,index)=>{let width=track.clientWidth;viewport.querySelectorAll<HTMLElement>(`.code-cell:nth-child(${index+1}) code`).forEach(code=>width=Math.max(width,code.scrollWidth+46));if(track.firstElementChild)(track.firstElementChild as HTMLElement).style.width=`${width}px`;});};
  if(viewport){bindSplitDiffScroll(viewport,tracks);if(tokens)applySourceTokens(viewport,tokens);if(typeof ResizeObserver!=='undefined'){observer=new ResizeObserver(size);observer.observe(viewport);}size();}
 }
 return {render:()=>stashView(view),renderDialog:()=>stashDialog(dialog,hooks.files(),view.busy,dialogError),bind,refresh,reset,showSave,isOpen:()=>!!dialog,isBusy:()=>view.busy};
}
