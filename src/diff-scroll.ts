export function bindSplitDiffScroll(viewport:HTMLElement,tracks:HTMLElement[]):void {
 tracks.forEach((track,index)=>track.onscroll=()=>viewport.style.setProperty(index?'--after-scroll':'--before-scroll',`${track.scrollLeft}px`));
 viewport.onwheel=event=>{
  if(event.ctrlKey||!tracks.length||(!event.deltaX&&!event.shiftKey))return;
  const bounds=viewport.getBoundingClientRect();
  const track=tracks[event.clientX<bounds.left+bounds.width/2?0:1];
  if(!track||track.scrollWidth<=track.clientWidth)return;
  const unit=event.deltaMode===1?16:event.deltaMode===2?track.clientWidth:1;
  const delta=(event.deltaX||event.deltaY)*unit;
  const next=Math.max(0,Math.min(track.scrollWidth-track.clientWidth,track.scrollLeft+delta));
  if(next===track.scrollLeft)return;
  event.preventDefault();track.scrollLeft=next;
  viewport.style.setProperty(track===tracks[0]?'--before-scroll':'--after-scroll',`${next}px`);
 };
}
