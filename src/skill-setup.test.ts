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
 document.getElementById('skills-install')!.click();await vi.waitFor(()=>expect(invoke).toHaveBeenCalledWith('skill_setup',{request:{op:'finish',targets:['agents']}}));
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
