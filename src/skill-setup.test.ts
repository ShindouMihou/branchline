// @vitest-environment happy-dom
import {vi,it,expect} from 'vitest';
const {invoke}=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke,isTauri:()=>true}));
import {createSkillSetup} from './skill-setup';
it('offers a readable guide and waits for explicit installation, with only selected targets',async()=>{
 const targets=[{id:'agents',name:'Codex & Cursor',path:'/home/.agents/skills/branchline/SKILL.md',status:'available',detail:'Available globally'},{id:'claude',name:'Claude Code',path:'/home/.claude/skills/branchline/SKILL.md',status:'conflict',detail:'Existing skill kept'}];
 invoke.mockImplementation(async(_name,{request})=>request.op==='status'?{shouldPrompt:true,targets,content:'<read before install>'}:{results:[{id:'agents',ok:true}]});
 let setup:ReturnType<typeof createSkillSetup>;const paint=()=>{document.body.innerHTML=setup.render();setup.bind()};setup=createSkillSetup(paint);
 await setup.show(true);expect(invoke.mock.calls.every(([,args])=>args.request.op==='status')).toBe(true);
 expect(document.querySelector('.skill-preview')?.textContent).toContain('<read before install>');expect(document.querySelector('.skill-preview read')).toBeNull();
 expect((document.querySelector('[data-skill-target="claude"]') as HTMLInputElement).disabled).toBe(true);
 document.getElementById('skills-install')!.click();await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('skill_setup',{request:{op:'finish',targets:['agents'],instructions:[]}}));
 await vi.waitFor(()=>expect(document.getElementById('skills-title')?.textContent).toContain('Your AI has the guide'));
});
it('does not show the automatic prompt again after dismissal',async()=>{
 invoke.mockReset();invoke.mockResolvedValue({shouldPrompt:false,targets:[],content:''});const setup=createSkillSetup(()=>{});await setup.show(true);expect(setup.isOpen()).toBe(false);expect(invoke).toHaveBeenCalledTimes(1);
});

it('minimizes and resumes without installing or recording a dismissal',async()=>{
 invoke.mockReset();invoke.mockResolvedValue({shouldPrompt:true,targets:[],content:'Guide'});
 let setup:ReturnType<typeof createSkillSetup>;const paint=()=>{document.body.innerHTML=setup.render();setup.bind()};setup=createSkillSetup(paint);await setup.show(true);
 document.getElementById('skills-minimize')!.click();expect(setup.isOpen()).toBe(false);expect(document.querySelector('.skills-backdrop')).toBeNull();expect(document.getElementById('skills-resume')).not.toBeNull();
 document.getElementById('skills-resume')!.click();expect(setup.isOpen()).toBe(true);expect(document.querySelector('.skills-backdrop')).not.toBeNull();expect(invoke).toHaveBeenCalledTimes(1);
});

it('adds reminders only when selected and allows reminder-only setup for installed guides',async()=>{
 invoke.mockReset();
 const target={id:'agents',name:'Codex',path:'/home/.agents/skills/branchline/SKILL.md',status:'installed',detail:'Installed'};
 const reminder={id:'codex',name:'Codex',skillTargetId:'agents',path:'/home/.codex/AGENTS.md',status:'available',detail:'Optional reminder',removable:false,content:'<!-- branchline:integration:start -->\n<keep safe>\n<!-- branchline:integration:end -->'};
 invoke.mockImplementation(async(_name,{request})=>request.op==='status'?{shouldPrompt:true,targets:[target],instructions:[reminder],content:'Guide'}:{results:[{id:'instructions:codex',ok:true}]});
 let setup:ReturnType<typeof createSkillSetup>;const paint=()=>{document.body.innerHTML=setup.render();setup.bind()};setup=createSkillSetup(paint);await setup.show();
 const checkbox=document.querySelector<HTMLInputElement>('[data-instruction-target]')!;
 expect(checkbox.checked).toBe(false);expect(document.querySelector('keep')).toBeNull();expect(document.querySelector('.instruction-preview')?.textContent).toContain('/home/.codex/AGENTS.md');
 expect((document.getElementById('skills-install') as HTMLButtonElement).disabled).toBe(true);
 expect(invoke.mock.calls.every(([,args])=>args.request.op==='status')).toBe(true);
 checkbox.checked=true;checkbox.dispatchEvent(new Event('change'));
 document.getElementById('skills-install')!.click();
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('skill_setup',{request:{op:'finish',targets:[],instructions:['codex']}}));
});

it('requires separate confirmation to remove a managed reminder',async()=>{
 invoke.mockReset();
 const reminder={id:'claude',name:'Claude Code',skillTargetId:'claude',path:'/home/.claude/CLAUDE.md',status:'installed',detail:'Installed',removable:true,content:'Reminder'};
 invoke.mockImplementation(async(_name,{request})=>request.op==='status'?{shouldPrompt:false,targets:[],instructions:[reminder],content:'Guide'}:{removed:true});
 let setup:ReturnType<typeof createSkillSetup>;const paint=()=>{document.body.innerHTML=setup.render();setup.bind()};setup=createSkillSetup(paint);await setup.show();
 document.querySelector<HTMLButtonElement>('[data-instruction-confirm]')!.click();
 expect(invoke.mock.calls.every(([,args])=>args.request.op==='status')).toBe(true);
 document.querySelector<HTMLButtonElement>('[data-instruction-cancel]')!.click();expect(document.querySelector('[data-instruction-remove]')).toBeNull();
 document.querySelector<HTMLButtonElement>('[data-instruction-confirm]')!.click();document.querySelector<HTMLButtonElement>('[data-instruction-remove]')!.click();
 await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('skill_setup',{request:{op:'remove_instructions',target:'claude',confirmed:true}}));
});
