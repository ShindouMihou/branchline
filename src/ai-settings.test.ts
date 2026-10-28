// @vitest-environment happy-dom
import {beforeEach,expect,it} from 'vitest';
import {aiSettings} from './ai-settings';
import type {AiSettingsView} from './types/ai-settings';
const view:AiSettingsView={enabled:true,autoOpen:true,notifications:true,autoApprovalCount:0,busy:false,selectedClientId:'',confirmDisconnect:false,clients:[{id:'test',name:'Local verification client',provider:'codex',active:true},{id:'gpt',name:'ChatGPT Desktop',provider:'chatgpt-desktop',active:true},{id:'code',name:'Claude Code',provider:'claude-code',active:true},{id:'desktop',name:'Claude Desktop',provider:'claude-desktop',active:true}]};
beforeEach(()=>{document.body.innerHTML='';});
it('keeps four connections compact and hides per-client controls on the overview',()=>{
 document.body.innerHTML=aiSettings(view);
 expect(document.querySelectorAll('.ai-settings-connection')).toHaveLength(4);
 expect(document.querySelector('[data-ai-client-type],[data-ai-read-source],[data-ai-revoke],[data-ai-setup-client]')).toBeNull();
 expect(document.querySelector('.ai-connection-heading')).toBeNull();expect(document.body.textContent).not.toContain('All local repositories');
 expect(document.getElementById('ai-auto-settings')?.textContent).toContain('Ask every time');
 expect(document.querySelectorAll('.ai-provider-logo')).toHaveLength(4);
 expect(document.getElementById('ai-notifications')?.getAttribute('aria-label')).toBe('Desktop notifications');
});
it('opens a focused client page with setup, classification and source access',()=>{
 document.body.innerHTML=aiSettings({...view,selectedClientId:'test'});
 expect(document.querySelector('.ai-connection-identity')?.textContent).toContain('Local verification client');
 expect(document.querySelector('[data-ai-client-type] option[selected]')?.getAttribute('value')).toBe('codex');
 expect(document.querySelector('[data-ai-read-source]')).not.toBeNull();expect(document.querySelector('[data-ai-setup-client]')?.textContent).toContain('Open guide');expect(document.querySelector('[data-ai-setup-client]')?.tagName).toBe('BUTTON');expect(document.querySelector('[data-ai-setup-client] button')).toBeNull();
 expect(document.querySelector('[data-ai-revoke]')).toBeNull();expect(document.getElementById('ai-disconnect-ask')).toBeNull();expect(document.getElementById('ai-connection-menu')?.getAttribute('aria-haspopup')).toBe('menu');
 expect(document.querySelector('.ai-settings-connections')).toBeNull();
});
it('shows an explicit disconnect action only in its confirmation state',()=>{
 document.body.innerHTML=aiSettings({...view,selectedClientId:'code',confirmDisconnect:true});
 expect(document.querySelector('[data-ai-revoke]')?.getAttribute('data-ai-revoke')).toBe('code');
 expect(document.getElementById('ai-disconnect-cancel')).not.toBeNull();
});
it('does not duplicate provider names and escapes user-controlled names',()=>{
 document.body.innerHTML=aiSettings({...view,clients:[{id:'x',name:'<img src=x>',provider:'codex',active:true},{id:'gpt',name:'ChatGPT Desktop',provider:'chatgpt-desktop',active:true}]});
 expect(document.querySelector('img')).toBeNull();expect(document.querySelectorAll('.ai-settings-connection small')).toHaveLength(1);
});
it('handles zero connections and connector-disabled state without hiding settings',()=>{
 document.body.innerHTML=aiSettings({...view,enabled:false,clients:[]});
 expect(document.querySelector<HTMLButtonElement>('#ai-add-connection')?.disabled).toBe(true);
 expect(document.getElementById('ai-skills')).not.toBeNull();expect(document.querySelector('.ai-settings-empty')?.textContent).toContain('Add your AI');
});

it('keeps disconnect in the connection actions menu rather than the page footer',()=>{
 document.body.innerHTML=aiSettings({...view,selectedClientId:'code',connectionMenuOpen:true});
 expect(document.querySelector('[role="menu"] #ai-disconnect-ask')).not.toBeNull();
 expect(document.querySelector('.ai-settings-disconnect')).toBeNull();
});
