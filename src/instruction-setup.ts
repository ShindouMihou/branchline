import {escapeHtml as e} from './html';
import {installableTargets} from './skill-installation';
import type {InstructionSetupOptions} from './types/skill-setup';

export function instructionSetup({targets,selected,disabled=false}:InstructionSetupOptions):string {
 if(!targets.length)return '';
 const pending=installableTargets(targets),installed=targets.every(target=>target.status==='installed');
 return `<section class="instruction-setup" aria-label="Optional AI reminders"><label class="skill-target"><i data-lucide="file-text" aria-hidden="true"></i><span><strong>Instruction reminder</strong><small>${installed?'Installed for all supported AI clients':'Add to all available, supported AI instruction files'}</small></span><input type="checkbox" data-instruction-all aria-label="Install instruction reminder for all available AI clients" ${installed||pending.length&&pending.every(target=>selected.has(target.id))?'checked':''} ${disabled||!pending.length?'disabled':''}></label><details class="instruction-preview"><summary>Preview reminder & locations</summary><p class="subtle">Adds only Branchline’s section. Custom instructions are preserved.</p>${targets.map(target=>`<div><strong>${e(target.name)}</strong><code>${e(target.path)}</code><p class="subtle">${e(target.detail)}</p></div>`).join('')}<pre>${e(targets[0].content)}</pre></details></section>`;
}
