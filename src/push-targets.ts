import {escapeHtml as e} from './html';
import type {AiCommandDetail} from './types/ai-command-details';

const refLabel = (reference:string) => reference.replace(/^refs\/(heads|tags)\//,'');

export function pushTargets(detail:AiCommandDetail) {
  const targets = detail.pushTargets ?? [];
  const atomic = detail.atomic === true ? '<p class="ai-push-atomic">All or nothing<span>Requests all destinations update together. Fails if the server does not support it.</span></p>' : '';
  const incomplete = detail.pushTargetsIncomplete ? '<p class="ai-step-copy">Some destinations need exact-command review.</p>' : '';
  if(!targets.length)return atomic+incomplete;
  return `${atomic}<div class="ai-command-field"><span>${targets.length===1?'Destination':`${targets.length} destinations`}</span><ul class="ai-push-targets">${targets.map(target=>{
    const label=target.kind==='tag'?'Tag':target.kind==='branch'?'Branch':'Ref';
    const glyph=target.kind==='tag'?'tag':'git-branch';
    const source=target.sourceRef==='HEAD'&&target.sourceBranch?`${target.sourceBranch} (HEAD)`:refLabel(target.sourceRef);
    return `<li><span class="ai-push-kind"><i data-lucide="${glyph}" aria-hidden="true"></i>${label}</span><div class="ai-command-route"><code title="${e(target.sourceRef)}">${e(source)}</code><i data-lucide="arrow-right" aria-label="to"></i><code title="${e(target.targetRef)}">${e(refLabel(target.targetRef))}</code></div></li>`;
  }).join('')}</ul>${incomplete}</div>`;
}
