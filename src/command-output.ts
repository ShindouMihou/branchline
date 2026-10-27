import {escapeHtml as e} from './html';
import type {CommandOutput} from './types/command-output';

export function terminalText(text:string){
  return text.replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g,'').replace(/\x1b\[[0-?]*[ -/]*[@-~]/g,'').replace(/\x1b[@-_]/g,'').replace(/\r\n/g,'\n').split('\n').map(line=>line.replace(/\r$/,'').split('\r').at(-1)??'').join('\n').replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g,'');
}

export function commandOutputPanel(index:number,running:boolean,output?:CommandOutput){
  return `<section class="ai-live-output" data-command-output="${index}" aria-label="Command output"><header><span><i data-lucide="terminal" aria-hidden="true"></i><strong>Command output</strong></span><small data-output-state>${running?'Live':'Finished'}</small></header><p class="ai-output-privacy">Only in this app session · excluded from AI responses and exports</p><div class="ai-output-console" data-ai-scroll="command-output-${index}" tabindex="0" aria-label="Scrollable command output"><p data-output-empty ${output&&(output.stdout||output.stderr)?'hidden':''}>${running?'Waiting for console output. Commands may buffer output or redirect it to a log file.':output?'The command produced no console output.':'Output is unavailable for this command in this app session.'}</p><div data-output-stream="stdout" ${output?.stdout?'':'hidden'}><small>Standard output</small><pre>${e(terminalText(output?.stdout??''))}</pre></div><div data-output-stream="stderr" ${output?.stderr?'':'hidden'}><small>Diagnostics</small><pre>${e(terminalText(output?.stderr??''))}</pre></div></div><small data-output-truncated ${output&&(output.stdoutTruncated||output.stderrTruncated)?'':'hidden'}>Showing the latest 128 KiB per stream. The command continues to run normally.</small></section>`;
}

export function updateCommandOutputs(root:ParentNode,outputs:CommandOutput[]){
  root.querySelectorAll<HTMLElement>('[data-command-output]').forEach(panel=>{
    const output=outputs.find(item=>item.stepIndex===Number(panel.dataset.commandOutput));
    if(!output)return;
    const console=panel.querySelector<HTMLElement>('.ai-output-console')!;
    const follow=console.scrollHeight-console.clientHeight-console.scrollTop<32;
    for(const stream of ['stdout','stderr'] as const){
      const section=panel.querySelector<HTMLElement>(`[data-output-stream="${stream}"]`)!;
      const text=terminalText(output[stream]);
      section.hidden=!text;
      const pre=section.querySelector('pre')!;
      if(pre.textContent!==text)pre.textContent=text;
    }
    panel.querySelector<HTMLElement>('[data-output-empty]')!.hidden=!!(output.stdout||output.stderr);
    panel.querySelector<HTMLElement>('[data-output-truncated]')!.hidden=!(output.stdoutTruncated||output.stderrTruncated);
    if(follow)console.scrollTop=console.scrollHeight;
  });
}
