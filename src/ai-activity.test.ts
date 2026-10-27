// @vitest-environment happy-dom
import {expect,it,vi} from 'vitest';
import {canClearActivity,matchesInbox} from './ai-activity';
import {createAiWorkspace,activityForAi} from './ai';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async()=>()=>{})}));
vi.mock('@tauri-apps/plugin-dialog',()=>({save:vi.fn()}));
it('limits Needs you to approvals and protects all live request states from history clearing',()=>{
 for(const status of ['pending_review','ready','approved','running'])expect(canClearActivity(status)).toBe(false);
 for(const status of ['succeeded','failed','cancelled','partially_completed','blocked','needs_review','outcome_unknown'])expect(canClearActivity(status)).toBe(true);
 for(const status of ['ready','approved','running','blocked','needs_review','partially_completed','succeeded'])expect(matchesInbox(status,'waiting')).toBe(false);
 expect(matchesInbox('pending_review','waiting')).toBe(true);
 expect(matchesInbox('blocked','activity')).toBe(true);
 expect(canClearActivity('future_unknown_state')).toBe(false);
});
it('moves blocked requests to Activity, confirms clearing, handles errors, and preserves pending approvals',async()=>{
 type Plan=Parameters<typeof activityForAi>[0];
 const base:Plan={id:'waiting',digest:'digest',clientName:'AI',reportedAi:null,repository:'/repo',status:'pending_review',createdAt:1,issue:null,explanation:{summary:'Check status',reason:'Read local changes',impact:'Inspect only',uncertainties:[]},requestedSteps:[{kind:'command.execute',program:'shell',argv:['-c','git status']}],preview:{branch:'main',files:[],patch:'',warnings:[],risks:[],push:null},approval:null,execution:[],events:[]};
 let plans:Plan[]=[base,{...base,id:'blocked',status:'blocked',preview:null,issue:'No executable plan'},{...base,id:'done',status:'succeeded'},{...base,id:'running',status:'running'}];
 let fail=true;
 invoke.mockImplementation(async(_command:string,{request}:{request:{op:string;plan_id?:string;confirmed?:boolean}})=>{
  if(request.op==='clear_activity'){
   expect(request.confirmed).toBe(true);
   if(fail){fail=false;throw new Error('Could not save activity');}
   plans=plans.filter(plan=>!canClearActivity(plan.status));return {cleared:2};
  }
  return {enabled:true,pending:1,clients:[],plans,selectedPlan:plans.find(plan=>plan.id===request.plan_id)??null};
 });
 let ui:ReturnType<typeof createAiWorkspace>;
 const paint=()=>{document.body.innerHTML=ui.toolbar()+ui.render();ui.bind()};
 ui=createAiWorkspace({render:paint,root:()=>'/repo'});paint();ui.start();
 await vi.waitFor(()=>expect(document.getElementById('ai-requests')?.textContent).toContain('1'));
 document.getElementById('ai-requests')!.click();
 await vi.waitFor(()=>expect(document.getElementById('ai-approve')).not.toBeNull());
 expect([...document.querySelectorAll<HTMLElement>('[data-ai-plan]')].map(button=>button.dataset.aiPlan)).toEqual(['waiting']);
 document.getElementById('ai-filter-activity')!.click();
 await vi.waitFor(()=>expect(document.querySelector('[data-ai-plan="blocked"]')).not.toBeNull());
 expect(document.querySelector('[data-ai-plan="waiting"]')).toBeNull();
 expect(document.getElementById('ai-approve')).toBeNull();
 document.getElementById('ai-clear-activity')!.click();
 expect(document.activeElement?.id).toBe('ai-clear-cancel');
 expect(invoke.mock.calls.some(([,args])=>args.request.op==='clear_activity')).toBe(false);
 document.getElementById('ai-clear-cancel')!.click();expect(document.getElementById('ai-clear-confirm')).toBeNull();
 document.getElementById('ai-clear-activity')!.click();document.getElementById('ai-clear-confirm')!.click();
 await vi.waitFor(()=>expect(document.querySelector('[role=alert]')?.textContent).toContain('Could not save activity'));
 expect(document.querySelector('[data-ai-plan="blocked"]')).not.toBeNull();
 document.getElementById('ai-clear-confirm')!.click();
 await vi.waitFor(()=>expect(document.querySelector('[data-ai-plan="blocked"]')).toBeNull());
 expect(document.querySelector('[data-ai-plan="done"]')).toBeNull();expect(document.querySelector('[data-ai-plan="running"]')).not.toBeNull();
 expect(document.getElementById('ai-clear-activity')).toBeNull();
 document.getElementById('ai-filter-waiting')!.click();
 await vi.waitFor(()=>expect(document.getElementById('ai-approve')).not.toBeNull());
 expect(document.querySelector('[data-ai-plan="waiting"]')).not.toBeNull();
 window.dispatchEvent(new Event('pagehide'));
});
