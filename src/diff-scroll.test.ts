// @vitest-environment happy-dom
import {it,expect,vi,beforeEach,afterEach} from 'vitest';
import {bindSplitDiffScroll} from './diff-scroll';
let frames=new Map<number,FrameRequestCallback>(),nextFrame=0;
beforeEach(()=>{
 frames=new Map();nextFrame=0;
 vi.stubGlobal('requestAnimationFrame',vi.fn((callback:FrameRequestCallback)=>{frames.set(++nextFrame,callback);return nextFrame}));
 vi.stubGlobal('cancelAnimationFrame',vi.fn((id:number)=>frames.delete(id)));
});
afterEach(()=>vi.unstubAllGlobals());
function paint(){const pending=[...frames.values()];frames.clear();pending.forEach(callback=>callback(0))}
function wheel(options:WheelEventInit):WheelEvent {
 const event=new WheelEvent('wheel',options);
 Object.defineProperties(event,{clientX:{value:options.clientX??0},ctrlKey:{value:options.ctrlKey??false},shiftKey:{value:options.shiftKey??false}});
 return event;
}
function setup(rightWidth=1600){
 const viewport=document.createElement('div'),tracks=[document.createElement('div'),document.createElement('div')];
 viewport.getBoundingClientRect=vi.fn(()=>({left:0,width:800} as DOMRect));
 Object.defineProperties(viewport,{clientWidth:{value:800},scrollWidth:{value:2400}});
 tracks.forEach((track,index)=>{Object.defineProperty(track,'clientWidth',{value:400});Object.defineProperty(track,'scrollWidth',{value:index?rightWidth:1600})});
 bindSplitDiffScroll(viewport,tracks);return {viewport,tracks};
}
it('routes horizontal trackpad gestures to the hovered split side',()=>{
 const {viewport,tracks}=setup();
 const right=wheel({deltaX:140,clientX:600,cancelable:true});viewport.dispatchEvent(right);paint();
 expect(tracks[1].scrollLeft).toBe(140);expect(tracks[0].scrollLeft).toBe(0);expect(right.defaultPrevented).toBe(true);expect(viewport.style.getPropertyValue('--after-scroll')).toBe('140px');
 viewport.dispatchEvent(wheel({deltaX:60,clientX:100}));paint();expect(tracks[0].scrollLeft).toBe(60);
});
it('preserves vertical scrolling and zoom while containing unified horizontal scrolling',()=>{
 const {viewport,tracks}=setup();
 for(const options of [{deltaY:100},{deltaX:100,ctrlKey:true}]){const event=wheel({...options,cancelable:true});viewport.dispatchEvent(event);expect(event.defaultPrevented).toBe(false)}
 expect(tracks[0].scrollLeft).toBe(0);bindSplitDiffScroll(viewport,[]);
 const event=wheel({deltaX:100,cancelable:true});viewport.dispatchEvent(event);paint();expect(event.defaultPrevented).toBe(true);expect(viewport.scrollLeft).toBe(100);
});
it('supports shift scrolling and line deltas and contains momentum at the edge',()=>{
 const {viewport,tracks}=setup();viewport.dispatchEvent(wheel({deltaY:3,deltaMode:1,shiftKey:true}));paint();expect(tracks[0].scrollLeft).toBe(48);
 viewport.dispatchEvent(wheel({deltaX:2000}));paint();expect(tracks[0].scrollLeft).toBe(1200);
 const event=wheel({deltaX:100,cancelable:true});viewport.dispatchEvent(event);paint();expect(event.defaultPrevented).toBe(true);expect(tracks[0].scrollLeft).toBe(1200);
});
it('contains sideways momentum at both edges and over a side without overflow',()=>{
 const {viewport,tracks}=setup(400);
 for(const deltaX of [-80,2000,80]){const event=wheel({deltaX,cancelable:true});viewport.dispatchEvent(event);paint();expect(event.defaultPrevented).toBe(true)}
 const event=wheel({deltaX:80,clientX:600,cancelable:true});viewport.dispatchEvent(event);paint();expect(event.defaultPrevented).toBe(true);expect(tracks[1].scrollLeft).toBe(0);
});
it('batches a burst of wheel events into one paint without repeated layout measurements',()=>{
 const {viewport,tracks}=setup();const writes=vi.spyOn(viewport.style,'setProperty');
 for(let index=0;index<100;index++)viewport.dispatchEvent(wheel({deltaX:2,deltaY:1,cancelable:true}));
 expect(frames.size).toBe(1);expect(writes).not.toHaveBeenCalled();expect(viewport.getBoundingClientRect).toHaveBeenCalledTimes(1);
 paint();expect(tracks[0].scrollLeft).toBe(200);expect(viewport.scrollTop).toBe(100);expect(writes).toHaveBeenCalledTimes(2);
 tracks[0].dispatchEvent(new Event('scroll'));paint();expect(writes).toHaveBeenCalledTimes(2);
});
it('keeps pending wheel movement when a delayed native scroll event arrives',()=>{
 const {viewport,tracks}=setup();
 viewport.dispatchEvent(wheel({deltaX:100}));paint();
 viewport.dispatchEvent(wheel({deltaX:60}));tracks[0].dispatchEvent(new Event('scroll'));paint();
 expect(tracks[0].scrollLeft).toBe(160);
});
it('restores native scrollbar positions and removes old handlers and pending paints on resize',()=>{
 const {viewport,tracks}=setup();tracks[1].scrollLeft=90;tracks[1].dispatchEvent(new Event('scroll'));paint();
 expect(viewport.style.getPropertyValue('--after-scroll')).toBe('90px');
 viewport.dispatchEvent(wheel({deltaX:80}));expect(frames.size).toBe(1);
 bindSplitDiffScroll(viewport,tracks);expect(frames.size).toBe(0);
 viewport.dispatchEvent(wheel({deltaX:20}));paint();expect(tracks[0].scrollLeft).toBe(100);
});
it('continues unified gestures from a position reached with the native scrollbar',()=>{
 const {viewport}=setup();bindSplitDiffScroll(viewport,[]);
 viewport.scrollLeft=400;viewport.dispatchEvent(new Event('scroll'));
 viewport.dispatchEvent(wheel({deltaX:60}));paint();expect(viewport.scrollLeft).toBe(460);
});
