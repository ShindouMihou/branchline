import { escapeHtml as e } from './html';
import {fileLabel} from './file-label';
import type { AiCommandDetail } from './types/ai-command-details';

const unknown = 'Not determined before execution';
const value = (text?: string) => e(text === undefined ? unknown : text || 'Empty');
const field = (label: string, text?: string, prominent = false) => `<div class="ai-command-field ${prominent ? 'ai-command-primary' : ''}"><span>${e(label)}</span><p>${value(text)}</p></div>`;
const route = (label: string, source?: string, target?: string) => `<div class="ai-command-field"><span>${e(label)}</span><div class="ai-command-route"><code>${value(source)}</code><i data-lucide="arrow-right" aria-hidden="true"></i><code>${value(target)}</code></div></div>`;
function files(paths: string[] | undefined, detailIndex: number) {
  return `<div class="ai-command-field"><div class="ai-command-files-heading">${paths?.length?`<button id="ai-review-all-${detailIndex}" data-ai-review-changes="${detailIndex}" aria-label="Review ${paths.length} changed files"><span>${paths.length} file${paths.length===1?'':'s'}</span><small>View diff</small><i data-lucide="chevron-right" aria-hidden="true"></i></button>`:'<span>Files</span>'}</div>${paths === undefined ? `<p>${unknown}</p>` : paths.length ? `<ul class="ai-command-files">${paths.map((path,fileIndex) => `<li><button id="ai-change-${detailIndex}-${fileIndex}" data-ai-review-changes="${detailIndex}" data-ai-review-path="${e(path)}" aria-label="Review changes in ${e(path)}"><i data-lucide="file-code-2" aria-hidden="true"></i>${fileLabel(path)}<i data-lucide="chevron-right" aria-hidden="true"></i></button></li>`).join('')}</ul>` : '<p>No files listed</p>'}</div>`;
}
function sharedCommit(details:AiCommandDetail[],index:number){
  const detail=details[index];
  if(detail.kind!=='index.stage'||!detail.files?.length)return false;
  const commit=details.slice(index+1).find(next=>next.kind==='commit.create');
  if(!commit?.files?.length)return false;
  const staged=new Set(detail.files),committed=new Set(commit.files);
  return staged.size===committed.size&&[...staged].every(path=>committed.has(path));
}
export function commandDetails(details: AiCommandDetail[], commandIndex: number) {
  return details.map((detail, detailIndex) => {
    if(detail.commandIndex!==commandIndex)return '';
    if(detail.kind==='details_unavailable')return `<details class="ai-command-detail-unavailable" data-ai-disclosure="command-unavailable-${commandIndex}-${detailIndex}"><summary><i data-lucide="info" aria-hidden="true"></i>Review the exact command</summary><p>Structured details are not available for this command.</p>${detail.warnings.map(warning=>`<p>${e(warning)}</p>`).join('')}${detail.evidence?`<p>${e(detail.evidence)}</p>`:''}</details>`;
    const shared=sharedCommit(details,detailIndex);
    const commit = detail.kind === 'commit.create', pr = detail.kind === 'pr.create', push = detail.kind === 'remote.push';
    const branch = detail.kind === 'branch.switch' || detail.kind === 'branch.create';
    const family = commit ? 'commit' : pr ? 'pr' : push ? 'push' : branch ? 'branch' : detail.kind === 'index.stage' ? 'stage' : 'unknown';
    const evidenceText = [detail.evidence, ...detail.warnings].join(' ');
    const evidenceLabel = detail.evidenceKind ? ({current_snapshot:'Current snapshot',projected:'Projected',command:'From command',unknown:'Unknown'})[detail.evidenceKind] : /prospective|projected|later action/i.test(evidenceText) ? 'Projected' : /current.*(?:index|snapshot)|(?:index|snapshot).*current/i.test(evidenceText) ? 'Current snapshot' : 'From command';
    const glyph = commit ? 'git-commit-horizontal' : pr ? 'git-pull-request' : push ? 'arrow-up' : branch ? 'git-branch' : detail.kind === 'index.stage' ? 'files' : 'info';
    const content = commit ? `${field('Commit message', detail.message, true)}${files(detail.files,detailIndex)}` : pr ? `${field('Pull request title', detail.pullRequestTitle, true)}${route('Source → target', detail.headBranch, detail.baseBranch)}${field('Description', detail.body)}` : push ? `${field('Remote', detail.remote, true)}${route('Source → destination branch', detail.sourceBranch, detail.targetBranch)}` : branch ? route(detail.kind === 'branch.create' ? 'Start point → new branch' : 'Current → target branch', detail.sourceBranch, detail.targetBranch) : detail.kind === 'index.stage' ? shared?'<p class="ai-shared-files">Files listed with the commit below.</p>':files(detail.files,detailIndex) : '<p class="ai-step-copy">Structured details could not be determined. Review the exact command above.</p>';
    const inspectable=(commit||detail.kind==='index.stage')&&!!detail.files?.length;
    return `<section id="ai-command-detail-${detailIndex}" class="ai-command-detail ai-command-detail-${family} ${shared?'ai-command-detail-shared':''}" aria-label="${e(detail.title)}"><header class="ai-command-card-heading"><h3>${inspectable?`<button id="ai-card-review-${detailIndex}" data-ai-review-changes="${detailIndex}" aria-label="Review changes for ${e(detail.title)}">`:''}<span class="ai-command-detail-icon"><i data-lucide="${glyph}" aria-hidden="true"></i></span><span>${e(detail.title)}</span>${inspectable?'<i class="ai-card-chevron" data-lucide="chevron-right" aria-hidden="true"></i></button>':''}</h3><span class="ai-command-evidence-label">${e(evidenceLabel)}</span></header>${content}${(detail.warnings.length||detail.evidence)&&!shared?`<details class="ai-command-detail-notes" data-ai-disclosure="command-evidence-${commandIndex}-${detailIndex}"><summary><i data-lucide="info" aria-hidden="true"></i><span>Preview details</span></summary>${detail.warnings.map(warning=>`<p>${e(warning)}</p>`).join('')}${detail.evidence?`<p>${e(detail.evidence)}</p>`:''}</details>`:''}</section>`;

  }).join('');
}

export function commandOverview(details:AiCommandDetail[]){
  const actions=details.map((detail,index)=>({detail,index})).filter(({detail})=>detail.kind!=='details_unavailable');
  if(!actions.length)return '';
  const labels:Record<string,[string,string]>={'index.stage':['files','Stage'],'commit.create':['git-commit-horizontal','Commit'],'branch.create':['git-branch','Branch'],'branch.switch':['git-branch','Switch'],'remote.push':['arrow-up','Push'],'pr.create':['git-pull-request','Pull request']};
  return `<nav class="ai-action-overview" aria-label="Proposed actions">${actions.map(({detail,index})=>{const [glyph,label]=labels[detail.kind]??['terminal',detail.title];return `<button data-ai-detail-jump="${index}" aria-label="Inspect ${e(detail.title)}"><i data-lucide="${glyph}" aria-hidden="true"></i><span>${e(label)}</span>${detail.files?.length?`<small>${detail.files.length}</small>`:''}</button>`}).join('<span aria-hidden="true">→</span>')}</nav>`;
}
