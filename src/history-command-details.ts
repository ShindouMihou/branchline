import type {AiCommandDetail} from './types/ai-command-details';

type Field=(label:string,value?:string,prominent?:boolean)=>string;

export function historyCommandContent(detail:AiCommandDetail,field:Field):string|null {
  if(detail.kind==='stash.save')return `${field('Stash message',detail.message??'Git will generate a message',true)}${field('Changes to save',detail.includeUntracked?'Tracked and untracked files':'Tracked files')}`;
  if(detail.kind==='stash.apply')return `${field('Stash reference',detail.stashRef,true)}${field('After restoring',detail.operation==='pop'?'Remove the stash if applying succeeds':'Keep the stash')}`;
  if(detail.kind==='history.rebase')return detail.operation?field('Rebase action',detail.operation,true):`${field('New base',detail.baseRef??detail.upstreamRef,true)}${field('Replay commits after',detail.upstreamRef)}${detail.targetBranch?field('Branch to rebase',detail.targetBranch):''}`;
  if(detail.kind==='history.reset')return `${field('Target',detail.targetBranch,true)}${field('Reset mode',detail.operation)}${field('Working files',detail.operation==='mixed'||detail.operation==='soft'?'Kept':detail.operation==='hard'?'Tracked changes may be discarded':'Review the exact command')}`;
  return null;
}
