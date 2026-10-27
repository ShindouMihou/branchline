export interface SkillTarget {
  id:string;
  name:string;
  path:string;
  status:'available'|'installed'|'update'|'conflict';
  detail:string;
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
  autoUpdate?:SkillAutoUpdateReport|null;
}
