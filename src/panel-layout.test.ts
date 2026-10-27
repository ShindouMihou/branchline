import {it,expect} from 'vitest';
import {parseLayout,togglePanel,defaultLayout} from './panel-layout';
it('validates persisted sizes and keeps a content panel visible',()=>{
  expect(parseLayout('bad')).toEqual(defaultLayout);
  const p=parseLayout(JSON.stringify({sidebarWidth:9000,logRatio:-3,logCollapsed:true,diffCollapsed:true}));expect(p.sidebarWidth).toBe(700);expect(p.logRatio).toBe(.15);expect(p.diffCollapsed).toBe(false);
  togglePanel(p,'diff');expect(p.diffCollapsed).toBe(true);expect(p.logCollapsed).toBe(false);
  togglePanel(p,'log');expect(p.logCollapsed).toBe(true);expect(p.diffCollapsed).toBe(false);
  togglePanel(p,'sidebar');expect(p.sidebarCollapsed).toBe(true);
});
