// @vitest-environment happy-dom
import {it,expect,vi} from 'vitest';
import {bindPanelResizers,defaultLayout} from './panel-layout';
it('resizes panels with the keyboard and saves bounded values',()=>{
  document.body.innerHTML='<main class="workspace"><div data-resize-panel="sidebar"></div><section class="editor"><div data-resize-panel="log"></div></section></main>';
  const workspace=document.querySelector<HTMLElement>('.workspace')!;Object.defineProperty(workspace,'clientWidth',{value:1000});
  const layout={...defaultLayout};const save=vi.fn();bindPanelResizers(document,layout,save);
  const side=document.querySelector<HTMLElement>('[data-resize-panel="sidebar"]')!;side.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight'}));expect(layout.sidebarWidth).toBe(342);expect(workspace.style.getPropertyValue('--sidebar-width')).toBe('342px');
  side.dispatchEvent(new KeyboardEvent('keydown',{key:'End'}));expect(layout.sidebarWidth).toBe(640);
  const log=document.querySelector('[data-resize-panel="log"]')!;log.dispatchEvent(new KeyboardEvent('keydown',{key:'End'}));expect(layout.logRatio).toBe(.85);log.dispatchEvent(new MouseEvent('dblclick'));expect(layout.logRatio).toBe(.4);expect(save).toHaveBeenCalledTimes(4);
});
