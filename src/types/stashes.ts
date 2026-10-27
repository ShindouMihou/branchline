export interface StashEntry { reference:string; oid:string; message:string; branch:string; timestamp:number }
export interface StashFile { path:string; source:'tracked'|'untracked' }
export interface WorkingFile { path:string; index:string; worktree:string }
export type StashScope = 'all'|'unstaged'|'selected';
export interface StashPreview { token:string; files:WorkingFile[]; scope:StashScope }
export interface StashDiff { patch:string; before:string|null; after:string|null; notice?:string }
export interface StashResult { ok:boolean; outcome:string; message:string; recoveryRef?:string; conflicts?:string[]; stashOid?:string }
export interface StashImage { before:{dataUrl:string;bytes:number}|null; after:{dataUrl:string;bytes:number}|null; notice?:string }
export type StashDialog = {kind:'save';scope:StashScope;name:string;includeUntracked:boolean;paths:Set<string>;preview:StashPreview|null} | {kind:'restore';entry:StashEntry;pop:boolean;restoreIndex:boolean;preview:{token:string;dirty:boolean}|null} | {kind:'delete';entry:StashEntry};
export interface StashView { entries:StashEntry[];entry:StashEntry|null;files:StashFile[];path:string;baseline:string;staged:boolean;filter:string;loading:boolean;loadingDiff:boolean;error:string;result:StashResult|null;diff:StashDiff|null;split:boolean;image:StashImage|null;imageMode:boolean;busy:boolean }
export interface StashHooks { root:()=>string|null; files:()=>WorkingFile[]; render:()=>void; api:<T>(request:object)=>Promise<T>; changed:(result:StashResult)=>Promise<void>; openChanges:()=>void; setBusy:(busy:boolean)=>void }
