const bindings=new WeakMap<HTMLElement,()=>void>();

export function bindSplitDiffScroll(viewport:HTMLElement,tracks:HTMLElement[]):()=>void {
 bindings.get(viewport)?.();
 const bounds=viewport.getBoundingClientRect();
 const limits=tracks.map(track=>Math.max(0,track.scrollWidth-track.clientWidth));
 const positions=tracks.map(track=>track.scrollLeft);
 const widths=tracks.map(track=>track.clientWidth);
 const painted=positions.map(()=>NaN);
 const pending=positions.map(()=>false);
 const viewportLimit=Math.max(0,viewport.scrollWidth-viewport.clientWidth);
 let horizontal=viewport.scrollLeft,vertical=0,frame:number|null=null,unifiedPending=false;
 const paint=()=>{
  frame=null;
  if(tracks.length){
   tracks.forEach((track,index)=>{
    if(track.scrollLeft!==positions[index])track.scrollLeft=positions[index];
    if(painted[index]!==positions[index]){
     viewport.style.setProperty(index?'--after-scroll':'--before-scroll',`${positions[index]}px`);
     painted[index]=positions[index];
    }
    pending[index]=false;
   });
  }else {
   if(viewport.scrollLeft!==horizontal)viewport.scrollLeft=horizontal;
   unifiedPending=false;
  }
  if(vertical){viewport.scrollTop+=vertical;vertical=0;}
 };
 const schedule=()=>{if(frame===null)frame=requestAnimationFrame(paint);};
 tracks.forEach((track,index)=>track.onscroll=()=>{
  if(pending[index])return;
  positions[index]=Math.max(0,Math.min(limits[index],track.scrollLeft));
  if(painted[index]!==positions[index])schedule();
 });
 const wheel=(event:WheelEvent)=>{
  if(event.ctrlKey||(!event.deltaX&&!event.shiftKey))return;
  event.preventDefault();
  const index=event.clientX<bounds.left+bounds.width/2?0:1;
  const track=tracks[index];
  const width=widths[index]??bounds.width;
  const unit=event.deltaMode===1?16:event.deltaMode===2?width:1;
  const delta=(event.deltaX||event.deltaY)*unit;
  if(track){positions[index]=Math.max(0,Math.min(limits[index],positions[index]+delta));pending[index]=true;}
  else if(!tracks.length){horizontal=Math.max(0,Math.min(viewportLimit,horizontal+delta));unifiedPending=true;}
  if(!event.shiftKey&&event.deltaX)vertical+=event.deltaY*unit;
  schedule();
 };
 const nativeScroll=()=>{if(!tracks.length&&!unifiedPending)horizontal=viewport.scrollLeft;};
 viewport.addEventListener('scroll',nativeScroll,{passive:true});
 viewport.addEventListener('wheel',wheel,{passive:false});
 let disposed=false;
 const dispose=()=>{
  if(disposed)return;disposed=true;
  if(frame!==null){cancelAnimationFrame(frame);paint();}
  viewport.removeEventListener('wheel',wheel);
  viewport.removeEventListener('scroll',nativeScroll);
  tracks.forEach(track=>track.onscroll=null);
  bindings.delete(viewport);
 };
 bindings.set(viewport,dispose);
 return dispose;
}
