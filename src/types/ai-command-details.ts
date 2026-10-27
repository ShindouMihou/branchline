export interface AiCommandDetail {
  commandIndex: number;
  kind: string;
  title: string;
  message?: string;
  files?: string[];
  filePreviews?: {path:string;source:'index'|'worktree'}[];
  evidenceKind?: 'current_snapshot'|'projected'|'command'|'unknown';
  remote?: string;
  sourceBranch?: string;
  targetBranch?: string;
  baseBranch?: string;
  headBranch?: string;
  pullRequestTitle?: string;
  body?: string;
  workingDirectory?: string;
  repository?: string;
  visibility?: string;
  sourceDirectory?: string;
  topics?: string[];
  tag?: string;
  releaseTitle?: string;
  assets?: string[];
  notesFile?: string;
  warnings: string[];
  evidence: string;
}
