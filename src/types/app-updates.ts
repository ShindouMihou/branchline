export interface AppUpdateState {
  revision: number;
  currentVersion: string;
  automatic: boolean;
  phase: 'idle'|'checking'|'current'|'available'|'downloading'|'installing'|'error';
  available: {version:string;notes:string}|null;
  downloaded: number;
  total: number|null;
  lastCheckedAt: number|null;
  error: string|null;
}

export type AppUpdateRequest = {op:'status'} | {op:'check';manual:boolean} | {op:'preferences';automatic:boolean} | {op:'install';version:string};
