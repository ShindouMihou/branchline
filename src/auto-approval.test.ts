// @vitest-environment happy-dom
import {beforeEach,expect,it,vi} from 'vitest';
import {autoApprovalBanner,autoApprovalSummary,createAutoApprovalSettings} from './auto-approval';
import type {AutoApprovalGrant,AutoApprovalScope} from './types/auto-approval';
const clients=[{id:'connection',name:'My AI',active:true}];
const rule:AutoApprovalGrant={id:'rule',revision:1,clientId:'connection',repository:'/repo',sessionWide:true,operations:[],remote:null,remoteUrl:null,branch:null,githubRepository:null,base:null,grantedAt:1,expiresAt:null};
beforeEach(()=>{document.body.innerHTML='';});
function harness(grants:AutoApprovalGrant[]=[],pickRepository=vi.fn<()=>Promise<string|null>>().mockResolvedValue(null)){
  const call=vi.fn().mockImplementation(async request=>request.op==='auto_approval_preview'?{scope:{...rule,...request.request,remoteUrl:request.request.operations.includes('push')?'https://github.com/example/project.git':null},digest:'reviewed-scope'}:{});
  let result:ReturnType<typeof createAutoApprovalSettings>;
  const render=()=>{document.body.innerHTML=autoApprovalBanner(grants)+result.render(grants,clients,true,false);result.bind();};
  result=createAutoApprovalSettings({render,run:fn=>void fn(),call,root:()=>'/repo',pickRepository});render();return {call,result};
}
const click=(id:string)=>document.getElementById(id)!.click();
function field(name:string,value:string){const input=document.querySelector<HTMLInputElement|HTMLSelectElement>(`[data-auto-field="${name}"]`)!;input.value=value;input.dispatchEvent(new Event(input.tagName==='SELECT'?'change':'input'));}
it('defaults to manual with no automatic permission or banner',()=>{const {call}=harness();expect(document.body.textContent).toContain('Ask every time');expect(document.querySelector('.ai-auto-banner')).toBeNull();expect(call).not.toHaveBeenCalled();click('ai-auto-add');expect(document.querySelector('[data-auto-mode="false"]')?.getAttribute('aria-checked')).toBe('true');expect((document.getElementById('ai-auto-review') as HTMLButtonElement).disabled).toBe(true);});
it('requires a separate reviewed scope and checked consent before enabling session approval',async()=>{
  const {call}=harness();click('ai-auto-add');document.querySelector<HTMLButtonElement>('[data-auto-mode="true"]')!.click();click('ai-auto-review');
  await vi.waitFor(()=>expect(document.body.textContent).toContain('All command requests'));expect(document.body.textContent).toContain('not a sandbox');
  field('minutes','30');
  await vi.waitFor(()=>expect(call.mock.calls.filter(([r])=>r.op==='auto_approval_preview')).toHaveLength(2));
  expect((document.getElementById('ai-auto-enable') as HTMLButtonElement).disabled).toBe(true);
  click('ai-auto-enable');expect(call.mock.calls.some(([r])=>r.op==='auto_approval_grant')).toBe(false);
  const consent=document.getElementById('ai-auto-confirm') as HTMLInputElement;consent.checked=true;consent.dispatchEvent(new Event('change'));click('ai-auto-enable');
  await vi.waitFor(()=>expect(call).toHaveBeenCalledWith({op:'auto_approval_grant',request:{clientId:'connection',repository:'/repo',allRepositories:false,sessionWide:true,operations:[],remote:'origin',branch:'',githubRepository:'',base:'main',minutes:30,confirmed:true,previewDigest:'reviewed-scope'}}));
});
it('shows push destination in confirmation and preserves entered text without moving focus',async()=>{
  harness();click('ai-auto-add');const push=document.querySelector<HTMLInputElement>('[data-auto-operation="push"]')!;push.checked=true;push.dispatchEvent(new Event('change'));
  const branch=document.querySelector<HTMLInputElement>('[data-auto-field="branch"]')!;branch.focus();field('branch','feat/test');expect(document.activeElement).toBe(branch);click('ai-auto-review');await vi.waitFor(()=>expect(document.querySelector('.ai-auto-summary')?.textContent).toContain('origin/feat/test'));expect(document.body.textContent).toContain('https://github.com/example/project.git');expect(document.body.textContent).toContain('whole requests');
});
it('offers immediate pause and individual rule removal without authorizing anything else',async()=>{
  const {call}=harness([rule]);expect(document.querySelector('.ai-auto-banner')?.textContent).toContain('Auto-approval on');click('ai-auto-pause');await vi.waitFor(()=>expect(call).toHaveBeenCalledWith({op:'auto_approval_pause'}));
  document.querySelector<HTMLButtonElement>('[data-auto-remove]')!.click();await vi.waitFor(()=>expect(call).toHaveBeenCalledWith({op:'auto_approval_remove',id:'rule'}));
});
it('escapes connection names, paths and branch names in rule summaries',()=>{
  const scope:AutoApprovalScope={clientId:'x',repository:'<script>bad</script>',sessionWide:false,operations:['push'],remote:'<img>',branch:'<test>',githubRepository:'',base:'',minutes:null};
  document.body.innerHTML=autoApprovalSummary(scope,[{id:'x',name:'<svg onload=bad>',active:true}]);expect(document.querySelector('script,img,svg')).toBeNull();expect(document.body.textContent).toContain('<test>');
});

it('requires a fresh preview and renewed consent after changing duration',async()=>{
 const {call}=harness();click('ai-auto-add');document.querySelector<HTMLButtonElement>('[data-auto-mode="true"]')!.click();click('ai-auto-review');
 await vi.waitFor(()=>expect(document.getElementById('ai-auto-confirm')).not.toBeNull());
 const consent=document.getElementById('ai-auto-confirm') as HTMLInputElement;consent.checked=true;consent.dispatchEvent(new Event('change'));
 expect((document.getElementById('ai-auto-enable') as HTMLButtonElement).disabled).toBe(false);
 field('minutes','15');await vi.waitFor(()=>expect(call.mock.calls.filter(([r])=>r.op==='auto_approval_preview')).toHaveLength(2));
 expect((document.getElementById('ai-auto-confirm') as HTMLInputElement).checked).toBe(false);
 expect((document.getElementById('ai-auto-enable') as HTMLButtonElement).disabled).toBe(true);
 expect(call.mock.calls.some(([r])=>r.op==='auto_approval_grant')).toBe(false);
});

it('keeps global scope opt-in and limits it to the selected connection',async()=>{
 const {call}=harness();click('ai-auto-add');
 expect(document.querySelector<HTMLSelectElement>('[data-auto-field="allRepositories"]')?.value).toBe('false');
 field('allRepositories','true');document.querySelector<HTMLButtonElement>('[data-auto-mode="true"]')!.click();click('ai-auto-review');
 await vi.waitFor(()=>expect(document.querySelector('.ai-auto-summary')?.textContent).toContain('All repositories'));
 expect(document.querySelector('.ai-auto-global-note')?.textContent).toContain('for this AI connection');
 expect(call.mock.calls.find(([r])=>r.op==='auto_approval_preview')?.[0].request).toMatchObject({clientId:'connection',allRepositories:true,confirmed:false});
 expect(call.mock.calls.some(([r])=>r.op==='auto_approval_grant')).toBe(false);
});

it('browses for a repository without switching the open workspace or granting authority',async()=>{
 const pick=vi.fn<()=>Promise<string|null>>().mockResolvedValue('/projects/another repo');const {call}=harness([],pick);click('ai-auto-add');field('allRepositories','browse');
 await vi.waitFor(()=>expect(document.querySelector<HTMLInputElement>('[data-auto-field="repository"]')?.value).toBe('/projects/another repo'));
 expect(pick).toHaveBeenCalledOnce();expect(call).not.toHaveBeenCalled();
 document.querySelector<HTMLButtonElement>('[data-auto-mode="true"]')!.click();click('ai-auto-review');
 await vi.waitFor(()=>expect(call).toHaveBeenCalledWith(expect.objectContaining({op:'auto_approval_preview',request:expect.objectContaining({repository:'/projects/another repo',allRepositories:false,confirmed:false})})));
});
it('keeps the previous repository scope when the folder dialog is cancelled',async()=>{
 const {call}=harness();click('ai-auto-add');field('allRepositories','true');field('allRepositories','browse');
 await vi.waitFor(()=>expect(document.querySelector('[data-auto-field="allRepositories"] option[selected]')?.getAttribute('value')).toBe('true'));
 expect(document.body.textContent).toContain('Applies to this AI connection only.');expect(call).not.toHaveBeenCalled();
});
