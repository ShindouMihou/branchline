// @vitest-environment happy-dom
import {expect,it} from 'vitest';
import {commandDetails} from './command-details';
import type {AiCommandDetail} from './types/ai-command-details';

it('shows every branch and tag destination with atomic behavior instead of unknown placeholders',()=>{
 const detail:AiCommandDetail={commandIndex:0,kind:'remote.push',title:'Push commits',remote:'origin',atomic:true,pushTargets:[{sourceRef:'HEAD',sourceBranch:'main',targetRef:'main',kind:'branch'},{sourceRef:'refs/tags/v0.4.1',targetRef:'refs/tags/v0.4.1',kind:'tag'}],warnings:[],evidence:'Literal arguments only'};
 document.body.innerHTML=commandDetails([detail],0);
 expect(document.querySelector('.ai-command-detail-push')?.textContent).toContain('origin');
 expect(document.querySelectorAll('.ai-push-targets li')).toHaveLength(2);
 expect(document.querySelector('.ai-push-atomic')?.textContent).toContain('All or nothing');
 expect(document.querySelector('.ai-push-atomic')?.textContent).toContain('server does not support it');
 const rows=document.querySelectorAll('.ai-push-targets li');
 expect(rows[0].textContent).toContain('main (HEAD)');
 expect(rows[1].textContent).toContain('Tag');
 expect(rows[1].querySelector('code')?.textContent).toBe('v0.4.1');
 expect(rows[1].querySelector('[data-lucide="tag"]')).not.toBeNull();
 expect(document.body.textContent).not.toContain('Not determined before execution');
});

it('preserves literal HEAD and escapes ref text when branch resolution is unavailable',()=>{
 document.body.innerHTML=commandDetails([{commandIndex:0,kind:'remote.push',title:'Push',remote:'origin',atomic:false,pushTargets:[{sourceRef:'HEAD',targetRef:'refs/heads/<test>',kind:'branch'}],warnings:[],evidence:''}],0);
 expect(document.querySelector('code')?.textContent).toBe('HEAD');
 expect(document.querySelectorAll('code')[1]?.textContent).toBe('<test>');
 expect(document.querySelector('test')).toBeNull();
 expect(document.querySelector('.ai-push-atomic')).toBeNull();
});

it('keeps incomplete destination coverage visible alongside known targets',()=>{
 document.body.innerHTML=commandDetails([{commandIndex:0,kind:'remote.push',title:'Push',remote:'origin',pushTargetsIncomplete:true,pushTargets:[{sourceRef:'HEAD',targetRef:'main',kind:'ref'}],warnings:[],evidence:''}],0);
 expect(document.querySelector('.ai-command-detail-push')?.textContent).toContain('Some destinations need exact-command review.');
 expect(document.querySelector('.ai-push-targets')?.textContent).toContain('main');
});
