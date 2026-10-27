export interface SkillTarget {
  id:string;
  name:string;
  path:string;
  status:'available'|'installed'|'update'|'conflict';
  detail:string;
}
export interface SkillInstructionTarget extends SkillTarget {
  skillTargetId:string;
  content:string;
  removable:boolean;
}
export interface InstructionSetupOptions {
  targets:SkillInstructionTarget[];
  selected:ReadonlySet<string>;
  disabled?:boolean;
  allowRemoval?:boolean;
  confirming?:string;
}
export interface SkillAutoUpdateReport {
  checkedAt:number;
  updatedCount:number;
  attentionCount:number;
  recordError?:string;
}
export interface SkillSetupState {
  shouldPrompt:boolean;
  targets:SkillTarget[];
  content:string;
  instructions?:SkillInstructionTarget[];
  autoUpdate?:SkillAutoUpdateReport|null;
}
