// @vitest-environment happy-dom
import {afterEach,beforeEach,expect,it,vi} from 'vitest';
import {createAppUpdates} from './app-updates';
import type {AppUpdateState} from './types/app-updates';
const {invoke,events}=vi.hoisted(()=>({invoke:vi.fn(),events:new Map<string,(event:{payload:AppUpdateState})=>void>()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async(name:string,callback:(event:{payload:AppUpdateState})=>void)=>{events.set(name,callback);return ()=>events.delete(name);})}));
let state:AppUpdateState;
beforeEach(()=>{
 invoke.mockReset();events.clear();document.body.innerHTML='';
 state={revision:1,currentVersion:'0.4.2',automatic:true,phase:'available',available:{version:'0.4.3',notes:'### Fixes\n\n- **Clear** updates\n\n<script>untrusted</script>'},downloaded:0,total:null,lastCheckedAt:1,error:null};
 invoke.mockImplementation(async(_command,{request})=>{
  if(request.op==='preferences')state={...state,revision:state.revision+1,automatic:request.automatic};
  if(request.op==='check')state={...state,revision:state.revision+1};
  if(request.op==='install')state={...state,revision:state.revision+1,phase:'downloading'};
  return state;
 });
});
afterEach(()=>{window.dispatchEvent(new Event('pagehide'));});
function workspace(notify:(message:string,failed:boolean)=>void=()=>{}){
 let ui:ReturnType<typeof createAppUpdates>;
 const paint=()=>{document.body.innerHTML=ui.toolbar()+(ui.isOpen()?ui.render():ui.settings());ui.bind();};
 ui=createAppUpdates(paint,()=>{},notify);ui.start();paint();return ui;
}
it('checks quietly and requires an explicit install click bound to the displayed version',async()=>{
 const ui=workspace();
 await vi.waitFor(()=>expect(document.getElementById('app-update-available')).not.toBeNull());
 expect(ui.isOpen()).toBe(false);
 expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'check',manual:false}});
 expect(invoke.mock.calls.some(([,args])=>args.request.op==='install')).toBe(false);
 document.getElementById('app-update-available')!.click();
 expect(document.querySelector('.app-update-notes h3')?.textContent).toBe('Fixes');
 expect(document.querySelector('script')).toBeNull();
 document.getElementById('app-updates-install')!.click();
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'install',version:'0.4.3'}}));
 await vi.waitFor(()=>expect((document.getElementById('app-updates-install') as HTMLButtonElement).disabled).toBe(true));
 expect(document.querySelector('progress')).not.toBeNull();
});
it('allows manual retry after failure and persists the automatic-check preference',async()=>{
 const ui=workspace();
 await vi.waitFor(()=>expect(document.getElementById('app-update-available')).not.toBeNull());
 ui.show();
 const input=document.getElementById('app-updates-automatic') as HTMLInputElement;
 input.checked=false;input.dispatchEvent(new Event('change'));
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'preferences',automatic:false}}));
 invoke.mockRejectedValueOnce(new Error('Offline'));
 document.getElementById('app-updates-check')!.click();
 await vi.waitFor(()=>expect(document.querySelector('[role="alert"]')?.textContent).toContain('Offline'));
 document.getElementById('app-updates-check')!.click();
 await vi.waitFor(()=>expect(document.querySelector('[role="alert"]')).toBeNull());
 expect((document.getElementById('app-updates-automatic') as HTMLInputElement).checked).toBe(false);
});
it('shares progress across windows and ignores stale update events',async()=>{
 const ui=workspace();
 await vi.waitFor(()=>expect(document.getElementById('app-update-available')).not.toBeNull());ui.show();
 events.get('branchline-app-update')!({payload:{...state,revision:4,phase:'downloading',downloaded:50,total:100}});
 events.get('branchline-app-update')!({payload:{...state,revision:2,phase:'available'}});
 expect(document.querySelector('progress')?.getAttribute('value')).toBe('50');
 expect((document.getElementById('app-updates-install') as HTMLButtonElement).disabled).toBe(true);
 document.getElementById('app-updates-close')!.click();expect(ui.isOpen()).toBe(false);
});

it('reports a failed manual check without opening a modal and keeps preferences accessible',async()=>{
 state={...state,phase:'error',available:null,error:'The update service is unavailable. Try again later.'};
 const notify=vi.fn();const ui=workspace(notify);
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'check',manual:false}}));
 ui.show(true);
 await vi.waitFor(()=>expect(notify).toHaveBeenCalledWith(state.error,true));
 expect(ui.isOpen()).toBe(false);
 expect(document.querySelector('.app-updates-modal')).toBeNull();
 expect(document.querySelector('[role="switch"]')).not.toBeNull();
 expect(document.querySelector('.appearance-update-heading')?.textContent).toContain('v0.4.2');
});

it('keeps an up-to-date manual check quiet and shows a nonmodal result',async()=>{
 state={...state,phase:'current',available:null};
 const notify=vi.fn();const ui=workspace(notify);
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'check',manual:false}}));
 ui.show();expect(ui.isOpen()).toBe(false);
 ui.show(true);
 await vi.waitFor(()=>expect(notify).toHaveBeenCalledWith('Branchline is up to date.',false));
 expect(document.querySelector('.app-updates-modal')).toBeNull();
 expect(document.getElementById('app-update-available')).toBeNull();
});

it('opens a manual-check prompt only after an available result arrives',async()=>{
 state={...state,phase:'current',available:null};
 const ui=workspace();
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'check',manual:false}}));
 invoke.mockImplementation(async(_command,{request})=>{
  if(request.op==='check'&&request.manual)state={...state,revision:state.revision+1,phase:'checking'};
  return state;
 });
 ui.show(true);
 await vi.waitFor(()=>expect((document.getElementById('app-updates-check') as HTMLButtonElement).disabled).toBe(true));
 expect(ui.isOpen()).toBe(false);
 events.get('branchline-app-update')!({payload:{...state,revision:state.revision+1,phase:'available',available:{version:'0.4.4',notes:'An update'}}});
 expect(ui.isOpen()).toBe(true);
 expect(document.querySelector('.app-update-status h3')?.textContent).toContain('0.4.4');
 expect(invoke.mock.calls.some(([,args])=>args.request.op==='install')).toBe(false);
});

it('does not mistake an unrelated window update for completion of a manual check',async()=>{
 state={...state,phase:'current',available:null};
 const notify=vi.fn();const ui=workspace(notify);
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('app_update',{request:{op:'check',manual:false}}));
 let complete:((next:AppUpdateState)=>void)|undefined;
 invoke.mockImplementation(()=>new Promise<AppUpdateState>(resolve=>{complete=resolve;}));
 ui.show(true);
 events.get('branchline-app-update')!({payload:{...state,revision:state.revision+1,automatic:false}});
 expect(notify).not.toHaveBeenCalled();
 complete!({...state,revision:state.revision+2,phase:'available',available:{version:'0.4.4',notes:'Update'}});
 await vi.waitFor(()=>expect(ui.isOpen()).toBe(true));
 expect(notify).not.toHaveBeenCalled();
});
