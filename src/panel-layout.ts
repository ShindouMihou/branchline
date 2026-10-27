export interface PanelLayout { sidebarWidth:number; logRatio:number; sidebarCollapsed:boolean; logCollapsed:boolean; diffCollapsed:boolean }
export const layoutKey='branchline.panel-layout.v1';
export const defaultLayout:PanelLayout={sidebarWidth:326,logRatio:.4,sidebarCollapsed:false,logCollapsed:false,diffCollapsed:false};
export function parseLayout(raw:string|null):PanelLayout {
  try {const p=JSON.parse(raw??'null');if(!p||typeof p!=='object')return {...defaultLayout};return {sidebarWidth:typeof p.sidebarWidth==='number'&&Number.isFinite(p.sidebarWidth)?Math.max(220,Math.min(700,p.sidebarWidth)):326,logRatio:typeof p.logRatio==='number'&&Number.isFinite(p.logRatio)?Math.max(.15,Math.min(.85,p.logRatio)):.4,sidebarCollapsed:p.sidebarCollapsed===true,logCollapsed:p.logCollapsed===true,diffCollapsed:p.logCollapsed===true?false:p.diffCollapsed===true};}catch{return {...defaultLayout}}
}
export function togglePanel(layout:PanelLayout,panel:'sidebar'|'log'|'diff') {
  const key=`${panel}Collapsed` as const;layout[key]=!layout[key];
  if(layout.logCollapsed&&layout.diffCollapsed)layout[panel==='log'?'diffCollapsed':'logCollapsed']=false;
}
let resizing=false;
export const isPanelResizing=()=>resizing;
export function bindPanelResizers(root:ParentNode,layout:PanelLayout,save:()=>void){
  root.querySelectorAll<HTMLElement>('[data-resize-panel]').forEach(handle=>{
    const sidebar=handle.dataset.resizePanel==='sidebar';
    const apply=(value:number)=>{const workspace=root.querySelector<HTMLElement>('.workspace')!,editor=root.querySelector<HTMLElement>('.editor')!;
      if(sidebar){const max=Math.max(220,Math.min(700,workspace.clientWidth-360));layout.sidebarWidth=Math.max(220,Math.min(max,value));workspace.style.setProperty('--sidebar-width',`${layout.sidebarWidth}px`);handle.setAttribute('aria-valuenow',String(Math.round(layout.sidebarWidth)));}
      else {layout.logRatio=Math.max(.15,Math.min(.85,value));editor.style.setProperty('--log-size',`${layout.logRatio*100}%`);handle.setAttribute('aria-valuenow',String(Math.round(layout.logRatio*100)));}
    };
    handle.addEventListener('pointerdown',event=>{
      if(event.button!==0)return;event.preventDefault();handle.setPointerCapture(event.pointerId);resizing=true;document.body.classList.add(sidebar?'resizing-columns':'resizing-rows');
      const container=root.querySelector<HTMLElement>(sidebar?'.workspace':'.editor')!;const bounds=container.getBoundingClientRect();
      const move=(e:PointerEvent)=>apply(sidebar?e.clientX-bounds.left:(e.clientY-bounds.top)/bounds.height);
      const finish=()=>{resizing=false;document.body.classList.remove('resizing-columns','resizing-rows');handle.removeEventListener('pointermove',move);handle.removeEventListener('pointerup',finish);handle.removeEventListener('pointercancel',finish);handle.removeEventListener('lostpointercapture',finish);save();};
      handle.addEventListener('pointermove',move);handle.addEventListener('pointerup',finish);handle.addEventListener('pointercancel',finish);handle.addEventListener('lostpointercapture',finish);
    });
    handle.addEventListener('keydown',event=>{
      const negative=sidebar?'ArrowLeft':'ArrowUp',positive=sidebar?'ArrowRight':'ArrowDown';if(![negative,positive,'Home','End'].includes(event.key))return;event.preventDefault();
      apply(event.key==='Home'?(sidebar?220:.15):event.key==='End'?(sidebar?700:.85):(sidebar?layout.sidebarWidth:layout.logRatio)+(event.key===negative?-1:1)*(sidebar?16:.025));save();
    });
    handle.addEventListener('dblclick',()=>{apply(sidebar?326:.4);save()});
  });
}
