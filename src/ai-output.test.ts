// @vitest-environment happy-dom
import {it,expect,vi} from 'vitest';
import {createAiWorkspace,activityForAi} from './ai';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
vi.mock('@tauri-apps/plugin-dialog',()=>({save:vi.fn(),open:vi.fn()}));
vi.mock('@tauri-apps/plugin-clipboard-manager',()=>({writeText:vi.fn()}));
vi.mock('@tauri-apps/api/event',()=>({listen:vi.fn(async()=>()=>{})}));
it('refreshes live output without repainting the request and keeps it out of receipts',async()=>{
  const p:Parameters<typeof activityForAi>[0]={id:'live',digest:'bound',clientName:'AI',reportedAi:null,repository:'/repo',status:'running',createdAt:1,issue:null,explanation:{summary:'Run checks and publish',reason:'Verify before publishing',impact:'Run the whole chain',uncertainties:[]},requestedSteps:[{kind:'command.execute',program:'shell',argv:['-c','node check && git push']}],preview:{branch:'main',files:[],patch:'',warnings:[],risks:[],push:null},approval:{at:1,through:1,authority:'native_user'},execution:[{kind:'command.execute',status:'running',result:null,error:null,commands:[{program:'/bin/sh',argv:['-c','node check && git push'],startedAt:1,finishedAt:null,outcome:null,stdinDigest:null}]}],events:[]};
  const output={stepIndex:0,stdout:'First progress',stderr:'',stdoutTruncated:false,stderrTruncated:false};
  invoke.mockImplementation(async(_command,{request})=>request.op==='status'?{enabled:true,pending:0,clients:[],plans:[p],selectedPlan:request.plan_id?p:null,commandOutputs:request.plan_id?[output]:[]}:{});
  let ui:ReturnType<typeof createAiWorkspace>;let paints=0;
  const paint=()=>{paints++;document.body.innerHTML=ui.toolbar()+ui.render();ui.bind()};
  ui=createAiWorkspace({render:paint,root:()=>'/repo'});paint();ui.start();
  document.getElementById('ai-requests')!.click();
  document.getElementById('ai-filter-activity')!.click();
  await vi.waitFor(()=>expect(document.querySelector('.ai-output-console')?.textContent).toContain('First progress'));
  const panel=document.querySelector('.ai-live-output');const previous=paints;
  output.stdout='First progress\nPrivate next progress';
  await vi.waitFor(()=>expect(document.querySelector('.ai-output-console')?.textContent).toContain('Private next progress'),{timeout:2000});
  expect(paints).toBe(previous);expect(document.querySelector('.ai-live-output')).toBe(panel);
  expect(activityForAi(p)).not.toContain('Private next progress');
  window.dispatchEvent(new Event('pagehide'));
});
