// @vitest-environment happy-dom
import {it,expect} from 'vitest';
import {bindSplitDiffScroll} from './diff-scroll';
function wheel(options:WheelEventInit):WheelEvent {
 const event=new WheelEvent('wheel',options);
 Object.defineProperties(event,{clientX:{value:options.clientX??0},ctrlKey:{value:options.ctrlKey??false},shiftKey:{value:options.shiftKey??false}});
 return event;
}
function setup(){
 const viewport=document.createElement('div'),tracks=[document.createElement('div'),document.createElement('div')];
 viewport.getBoundingClientRect=()=>({left:0,width:800} as DOMRect);
 for(const track of tracks){Object.defineProperty(track,'clientWidth',{value:400});Object.defineProperty(track,'scrollWidth',{value:1600})}
 bindSplitDiffScroll(viewport,tracks);return {viewport,tracks};
}
it('routes horizontal trackpad gestures to the hovered split side',()=>{
 const {viewport,tracks}=setup();
 const right=wheel({deltaX:140,clientX:600,cancelable:true});viewport.dispatchEvent(right);
 expect(tracks[1].scrollLeft).toBe(140);expect(tracks[0].scrollLeft).toBe(0);expect(right.defaultPrevented).toBe(true);expect(viewport.style.getPropertyValue('--after-scroll')).toBe('140px');
 viewport.dispatchEvent(wheel({deltaX:60,clientX:100}));expect(tracks[0].scrollLeft).toBe(60);
});
it('preserves vertical scrolling, zoom, and native scrolling without split tracks',()=>{
 const {viewport,tracks}=setup();
 for(const options of [{deltaY:100},{deltaX:100,ctrlKey:true}]){const event=wheel({...options,cancelable:true});viewport.dispatchEvent(event);expect(event.defaultPrevented).toBe(false)}
 expect(tracks[0].scrollLeft).toBe(0);bindSplitDiffScroll(viewport,[]);const event=wheel({deltaX:100,cancelable:true});viewport.dispatchEvent(event);expect(event.defaultPrevented).toBe(false);
});
it('supports shift scrolling and line deltas without swallowing gestures at the edge',()=>{
 const {viewport,tracks}=setup();viewport.dispatchEvent(wheel({deltaY:3,deltaMode:1,shiftKey:true}));expect(tracks[0].scrollLeft).toBe(48);
 viewport.dispatchEvent(wheel({deltaX:2000}));expect(tracks[0].scrollLeft).toBe(1200);
 const event=wheel({deltaX:100,cancelable:true});viewport.dispatchEvent(event);expect(event.defaultPrevented).toBe(false);
});
