import './command-input.css';
import {escapeHtml as e} from './html';
import type {AiCommandDetail} from './types/ai-command-details';

export function commandInput(input:string|null|undefined,commandIndex:number,details:AiCommandDetail[]):string {
 if(input==null)return '';
 const description=details.some(detail=>detail.commandIndex===commandIndex&&detail.bodySource==='stdin'&&detail.body===input);
 const label=description?'Description input':'Command input';
 const note=description?'Shown in the pull request description above. This is the exact text sent to GitHub CLI.':'This text is sent to the command through standard input.';
 return `<details class="ai-command-input" data-ai-disclosure="stdin-${commandIndex}"><summary><i data-lucide="file-text" aria-hidden="true"></i><span>${label}</span><small>${input.length.toLocaleString()} characters</small><i data-lucide="chevron-down" aria-hidden="true"></i></summary><div><p>${note}</p><pre tabindex="0" role="region" aria-label="Exact standard input">${e(input||'Empty input')}</pre></div></details>`;
}
