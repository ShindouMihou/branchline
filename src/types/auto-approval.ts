export type AutoApprovalOperation = 'stage'|'commit'|'push'|'pr_create'|'pr_edit';
export interface AutoApprovalGrant {
  id:string;
  revision:number;
  clientId:string;
  repository:string;
  allRepositories?:boolean;
  sessionWide:boolean;
  operations:AutoApprovalOperation[];
  remote:string|null;
  remoteUrl:string|null;
  branch:string|null;
  githubRepository:string|null;
  base:string|null;
  grantedAt:number;
  expiresAt:number|null;
  resolvedRepository?:string;
  resolvedPushUrl?:string;
}
export interface AutoApprovalConnection {id:string;name:string;active:boolean}
export interface AutoApprovalScope {
  clientId:string;
  repository:string;
  allRepositories?:boolean;
  sessionWide:boolean;
  operations:AutoApprovalOperation[];
  remote:string;
  branch:string;
  githubRepository:string;
  base:string;
  minutes:number|null;
}

export interface AutoApprovalPreview {scope:AutoApprovalGrant;digest:string}
