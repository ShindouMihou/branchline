// @vitest-environment happy-dom
import {expect,it} from 'vitest';
import {markdown} from './markdown';
import {commandDetails} from './command-details';

it('renders readable release notes with headings, lists, code, quotes and tables',()=>{
 document.body.innerHTML=markdown('### Highlights\n\n- **Clear approvals**\n- Review `git status`\n\n> Check the changes\n\n```sh\ngit status\ngit diff\n```\n\n| Action | Result |\n| --- | --- |\n| Commit | Saved |');
 expect(document.querySelector('h3')?.textContent).toBe('Highlights');
 expect(document.querySelectorAll('li')).toHaveLength(2);
 expect(document.querySelector('strong')?.textContent).toBe('Clear approvals');
 expect(document.querySelector('pre code')?.textContent).toBe('git status\ngit diff\n');
 expect(document.querySelectorAll('td')).toHaveLength(2);
 expect(document.querySelector('blockquote')?.textContent).toContain('Check the changes');
});

it('escapes active HTML and omits images, including images inside links',()=>{
 document.body.innerHTML=markdown('<script>alert(1)</script>\n\n<img src="https://tracker.test/pixel" onerror="alert(1)">\n\n![A screenshot](https://tracker.test/pixel)\n\n[![Nested](https://tracker.test/other)](https://example.com)');
 expect(document.querySelector('script,img,iframe,svg')).toBeNull();
 expect(document.body.textContent).toContain('<script>alert(1)</script>');
 expect(document.querySelectorAll('.markdown-image-label')).toHaveLength(2);
});

it('accepts only explicit HTTP and HTTPS links without credentials',()=>{
 document.body.innerHTML=markdown('[Safe](https://example.com/docs?q=git&v=1) [HTTP](http://localhost:3000) [JS](javascript:alert%281%29) [File](file:///etc/passwd) [Data](data:text/html,test) [Relative](/settings) [User](https://user:password@example.com)');
 const links=Array.from(document.querySelectorAll('a'));
 expect(links).toHaveLength(2);
 expect(links[0].dataset.markdownUrl).toBe('https://example.com/docs?q=git&v=1');
 expect(links[0].getAttribute('rel')).toBe('noopener noreferrer');
});

it('formats descriptions and release notes while preserving literal commit text',()=>{
 const common={commandIndex:0,warnings:[],evidence:''};
 document.body.innerHTML=commandDetails([
 {...common,kind:'release.create',title:'Release',body:'### Highlights\n\n- Ready'},
 {...common,kind:'pr.create',title:'Pull request',body:'**Review** this change'},
 {...common,kind:'commit.create',title:'Commit',message:'**Literal** `message`'}
 ],0);
 expect(document.querySelector('.markdown-content h3')?.textContent).toBe('Highlights');
 expect(document.querySelector('.ai-command-detail-pr strong')?.textContent).toBe('Review');
 expect(document.querySelector('.ai-command-detail-commit .ai-command-primary p')?.textContent).toBe('**Literal** `message`');
});
