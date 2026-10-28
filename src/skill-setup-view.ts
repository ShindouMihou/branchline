import {escapeHtml as e} from './html';
import {installationRequest} from './skill-installation';
import type {SkillSetupState,SkillTarget} from './types/skill-setup';

function installRow(title:string,description:string,icon:string,targets:SkillTarget[],id:string,enabled:boolean,busy:boolean){
 const installed=targets.length>0&&targets.every(target=>target.status==='installed');
 const conflicts=targets.some(target=>target.status==='conflict');
 const count=targets.filter(target=>target.status==='installed').length;
 return `<div class="skills-install-row"><i data-lucide="${icon}" aria-hidden="true"></i><div class="skills-install-copy"><strong>${title}</strong><p>${description}</p>${!installed&&count?`<small>${count} of ${targets.length} installed</small>`:''}${conflicts?'<small class="skills-attention">Custom files kept. See details below.</small>':''}</div>${installed?'<span class="skills-installed" role="status" title="Installed for every supported client detected on this Mac"><i data-lucide="check" aria-hidden="true"></i> Installed</span>':enabled?`<button id="${id}" class="ai-secondary-button" aria-label="${targets.some(target=>target.status==='update')?'Update':'Install'} ${title.toLowerCase()} for all supported AI clients" ${busy?'disabled':''}>${busy?'Working…':targets.some(target=>target.status==='update')?'Update all':'Install for all'}</button>`:'<span class="skills-unavailable">Needs attention</span>'}</div>`;
}
export function skillTargets(state:SkillSetupState,busy:boolean){
 const guides=installationRequest(state,false),reminders=installationRequest(state,true);
 return `<section class="skills-guides" aria-label="AI integration"><p class="skills-coverage">Help your AI use Branchline<small>${e(state.targets.map(target=>target.name).join(' · '))}</small></p><div class="skills-install-list">${installRow('Skills','Prepare clear requests and wait for results.','book-open',state.targets,'skills-install',!!guides.targets.length,busy)}${(state.instructions??[]).length?installRow('Instruction reminder','Remember to use Branchline for Git approvals.','file-text',state.instructions??[],'skills-install-reminders',!!reminders.instructions.length,busy):''}</div></section>`;
}
