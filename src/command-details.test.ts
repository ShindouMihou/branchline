// @vitest-environment happy-dom
import { expect, it } from 'vitest';
import { commandDetails } from './command-details';
import type { AiCommandDetail } from './types/ai-command-details';

it('groups structured command details by requested step and keeps unknowns explicit', () => {
  const details: AiCommandDetail[] = [
    {commandIndex:0,kind:'commit.create',title:'Commit changes',message:'Update <welcome>',files:['src/<welcome>.ts'],warnings:['Files may change before execution'],evidence:'Read from the command and current index'},
    {commandIndex:0,kind:'pr.create',title:'Create pull request',pullRequestTitle:'Improve welcome',headBranch:'feat/welcome',baseBranch:'main',body:'First line\n<script>description</script>',warnings:[],evidence:'Projected values for a prospective later action'},
    {commandIndex:0,kind:'branch.switch',title:'Switch branch',targetBranch:'next',warnings:[],evidence:''},
    {commandIndex:1,kind:'index.stage',title:'Stage files',files:['other.txt'],warnings:[],evidence:''}
  ];
  document.body.innerHTML=commandDetails(details,0);
  expect(document.querySelectorAll('.ai-command-detail')).toHaveLength(3);
  expect(document.body.textContent).toContain('Update <welcome>');
  expect(document.querySelector('.file-path')?.firstChild?.textContent).toBe('<welcome>.ts');
  expect(document.querySelector('.file-path small')?.textContent).toBe('src');
  expect(document.querySelector('[data-ai-review-path] [data-lucide]')?.getAttribute('data-lucide')).toBe('file-code-2');
  expect(document.body.textContent).toContain('First line\n<script>description</script>');
  expect(document.querySelector('script')).toBeNull();
  expect(document.querySelector('welcome')).toBeNull();
  expect(document.body.textContent).toContain('feat/welcome');
  expect(document.body.textContent).toContain('main');
  expect(document.body.textContent).toContain('Not determined before execution');
  expect(document.body.textContent).not.toContain('other.txt');
  expect(document.body.textContent).toContain('Files may change before execution');
  const labels=Array.from(document.querySelectorAll('.ai-command-evidence-label'),label=>label.textContent);
  expect(labels).toEqual(['Current snapshot','Projected','From command']);
});

it('shows push routes and keeps unsupported details compact', () => {
  const details: AiCommandDetail[] = [
    {commandIndex:0,kind:'remote.push',title:'Push changes',remote:'origin',sourceBranch:'feat/welcome',targetBranch:'feat/review',warnings:[],evidence:'Projected values after an earlier command'},
    {commandIndex:0,kind:'details_unavailable',title:'Review exact command',warnings:['Arguments are resolved by Git'],evidence:'Effects are not verified'}
  ];
  document.body.innerHTML=commandDetails(details,0);
  expect(document.querySelector('.ai-command-detail-push')?.textContent).toContain('origin');
  expect(document.querySelector('.ai-command-route')?.textContent).toContain('feat/welcome');
  expect(document.querySelector('.ai-command-route')?.textContent).toContain('feat/review');
  expect(document.querySelector('.ai-command-evidence-label')?.textContent).toBe('Projected');
  expect(document.querySelectorAll('.ai-command-detail')).toHaveLength(1);
  expect(document.querySelector('.ai-command-detail-unavailable summary')?.textContent).toContain('Review the exact command');
});

it('lists matching stage and commit files once, while keeping staging inspectable',()=>{
  const details:AiCommandDetail[]=[
    {commandIndex:0,kind:'index.stage',title:'Stage changes',files:['b.txt','a.txt'],warnings:[],evidence:'Projected'},
    {commandIndex:0,kind:'commit.create',title:'Commit changes',files:['a.txt','b.txt'],message:'Save changes',warnings:[],evidence:'Projected'}
  ];
  document.body.innerHTML=commandDetails(details,0);
  expect(document.querySelectorAll('.ai-command-files')).toHaveLength(1);
  expect(document.querySelector('.ai-command-detail-stage')?.textContent).toContain('Files listed with the commit below.');
  expect(document.querySelector('.ai-command-detail-stage [data-ai-review-changes]')?.getAttribute('data-ai-review-changes')).toBe('0');
  expect(document.querySelectorAll('.ai-command-detail-commit [data-ai-review-path]')).toHaveLength(2);
  details[1].files!.push('already-staged.txt');
  document.body.innerHTML=commandDetails(details,0);
  expect(document.querySelectorAll('.ai-command-files')).toHaveLength(2);
});
