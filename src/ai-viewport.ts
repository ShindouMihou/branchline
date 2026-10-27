interface Viewport {
  view:string;
  scroll:number;
  inboxScroll:number;
  openDetails:Set<string>;
  focusedStep:string|null;
  focusedId:string|null;
  areas:{key:string;top:number;left:number}[];
}
const detailKey=(el:HTMLDetailsElement)=>el.dataset.aiDisclosure??el.querySelector('summary')?.textContent??'';
export function captureAiViewport(root:ParentNode):Viewport|null {
  const modal=root.querySelector<HTMLElement>('.ai-modal');
  if(!modal)return null;
  const active=document.activeElement as HTMLElement|null;
  return {view:modal.dataset.aiView??'',scroll:modal.querySelector('.ai-review-scroll')?.scrollTop??0,inboxScroll:modal.querySelector('.ai-inbox-list')?.scrollTop??0,openDetails:new Set([...modal.querySelectorAll<HTMLDetailsElement>('details[open]')].map(detailKey)),focusedStep:modal.contains(active)?active?.dataset.aiStep??null:null,focusedId:modal.contains(active)?active?.id||null:null,areas:[...modal.querySelectorAll<HTMLElement>('[data-ai-scroll]')].map(el=>({key:el.dataset.aiScroll!,top:el.scrollTop,left:el.scrollLeft}))};
}
export function restoreAiViewport(root:ParentNode,state:Viewport|null){
  const modal=root.querySelector<HTMLElement>('.ai-modal');
  if(!state||!modal||modal.dataset.aiView!==state.view)return;
  // Restore content height before scrollTop: browsers clamp it while details are closed.
  modal.querySelectorAll<HTMLDetailsElement>('details').forEach(el=>el.open=state.openDetails.has(detailKey(el)));
  if(state.focusedStep!==null) [...modal.querySelectorAll<HTMLElement>('[data-ai-step]')].find(el=>el.dataset.aiStep===state.focusedStep)?.focus({preventScroll:true});
  if(state.focusedId)document.getElementById(state.focusedId)?.focus({preventScroll:true});
  for(const area of state.areas){const el=[...modal.querySelectorAll<HTMLElement>('[data-ai-scroll]')].find(item=>item.dataset.aiScroll===area.key);if(el){el.scrollTop=area.top;el.scrollLeft=area.left;if(el.hasAttribute('data-ai-diff-scroll'))el.dispatchEvent(new Event('scroll'));}}
  const review=modal.querySelector('.ai-review-scroll');if(review)review.scrollTop=state.scroll;
  const inbox=modal.querySelector('.ai-inbox-list');if(inbox)inbox.scrollTop=state.inboxScroll;
}
