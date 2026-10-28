import type {SkillSetupState,SkillTarget} from './types/skill-setup';

export function installableTargets<T extends SkillTarget>(targets:readonly T[]):T[]{
 return targets.filter(target=>target.status==='available'||target.status==='update');
}
export function reminderTargets(state:SkillSetupState){
 return installableTargets(state.instructions??[]).filter(target=>state.targets.some(skill=>skill.id===target.skillTargetId&&skill.status!=='conflict'));
}
export function installationRequest(state:SkillSetupState,reminders:boolean){
 const instructions=reminders?reminderTargets(state):[];
 const dependencies=new Set(instructions.map(target=>target.skillTargetId));
 return {op:'finish',targets:installableTargets(state.targets).filter(target=>!reminders||dependencies.has(target.id)).map(target=>target.id),instructions:instructions.map(target=>target.id)};
}
