// @vitest-environment happy-dom
import {expect,it} from 'vitest';
import {commandInput} from './command-input';
import {commandDetails,commandOverview} from './command-details';
import type {AiCommandDetail} from './types/ai-command-details';

const pr=(body:string):AiCommandDetail=>({commandIndex:0,kind:'pr.edit',title:'Update pull request',repository:'Pedi-Solutions-Inc/kusina',pullRequestTarget:'1',pullRequestTitle:'Rewrite stage prompts',body,bodySource:'stdin',warnings:[],evidence:'Requested updates'});
it('shows an edit card with PR identity and Markdown description without inventing branch changes',()=>{
 const body='## Summary\n\n- Use **fewer tokens**\n\n<script>untrusted</script>';
 document.body.innerHTML=commandOverview([pr(body)])+commandDetails([pr(body)],0)+commandInput(body,0,[pr(body)]);
 expect(document.querySelector('.ai-command-detail-pr h3')?.textContent).toBe('Update pull request');
 expect(document.body.textContent).toContain('#1');expect(document.body.textContent).toContain('Pedi-Solutions-Inc/kusina');
 expect(document.querySelector('.markdown-content h2')?.textContent).toBe('Summary');
 expect(document.querySelector('.markdown-content strong')?.textContent).toBe('fewer tokens');
 expect(document.querySelector('script')).toBeNull();expect(document.querySelector('.ai-command-route')).toBeNull();
 expect(document.querySelector('.ai-command-input summary')?.textContent).toContain('Description input');
 expect((document.querySelector('.ai-command-input') as HTMLDetailsElement).open).toBe(false);
 expect(document.querySelector('.ai-command-input pre')?.textContent).toBe(body);
 expect(document.querySelector('.ai-action-overview')?.textContent).toContain('Update PR');
});
it('renders generic stdin as a quiet disclosure with escaped exact text, including empty input',()=>{
 expect(commandInput(null,0,[])).toBe('');
 document.body.innerHTML=commandInput('<img src=x onerror=alert(1)>',2,[]);
 expect(document.querySelector('img')).toBeNull();expect(document.querySelector('summary')?.textContent).toContain('Command input');
 expect(document.querySelector('details')?.dataset.aiDisclosure).toBe('stdin-2');
 expect(document.querySelector('pre')?.textContent).toBe('<img src=x onerror=alert(1)>');
 document.body.innerHTML=commandInput('',0,[pr('')]);expect(document.querySelector('summary')?.textContent).toContain('0 characters');expect(document.querySelector('pre')?.textContent).toBe('Empty input');
});
it('does not label input as the PR description unless it matches the bound input and command step',()=>{
 document.body.innerHTML=commandInput('Different input',0,[pr('Original body')]);expect(document.querySelector('summary')?.textContent).toContain('Command input');
 document.body.innerHTML=commandInput('Original body',1,[pr('Original body')]);expect(document.querySelector('summary')?.textContent).toContain('Command input');
});
it('an edit with omitted title and description only displays its target and requested base change',()=>{
 const detail:AiCommandDetail={commandIndex:0,kind:'pr.edit',title:'Update pull request',pullRequestTarget:'feature',baseBranch:'next',warnings:[],evidence:''};
 document.body.innerHTML=commandDetails([detail],0);
 expect(document.body.textContent).toContain('New base branch');expect(document.body.textContent).not.toContain('New description');expect(document.body.textContent).not.toContain('New title');
});
