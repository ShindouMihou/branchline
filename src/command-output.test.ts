// @vitest-environment happy-dom
import {it,expect} from 'vitest';
import {commandOutputPanel,updateCommandOutputs,terminalText} from './command-output';

it('updates native output as text without losing the user’s horizontal or vertical scroll',()=>{
  document.body.innerHTML=commandOutputPanel(0,true);
  const console=document.querySelector<HTMLElement>('.ai-output-console')!;
  Object.defineProperties(console,{scrollHeight:{value:1000},clientHeight:{value:200}});
  console.scrollTop=120;console.scrollLeft=80;
  updateCommandOutputs(document,[{stepIndex:0,stdout:'<img src=x onerror=alert(1)>\n\x1b[32mPassed\x1b[0m',stderr:'warning',stdoutTruncated:false,stderrTruncated:true}]);
  expect(document.querySelector('pre')!.textContent).toContain('<img');
  expect(document.querySelector('img')).toBeNull();
  expect(document.querySelector('pre')!.textContent).toContain('Passed');
  expect(console.scrollTop).toBe(120);expect(console.scrollLeft).toBe(80);
  expect(document.querySelector<HTMLElement>('[data-output-truncated]')!.hidden).toBe(false);
  console.scrollTop=800;
  updateCommandOutputs(document,[{stepIndex:0,stdout:'next',stderr:'',stdoutTruncated:false,stderrTruncated:false}]);
  expect(console.scrollTop).toBe(1000);
});

it('removes terminal controls and explains redirected output without promising progress',()=>{
  expect(terminalText('\x1b]8;;https://example.com\x07link\x1b]8;;\x07\r\n10%\r20%')).toBe('link\n20%');
  expect(terminalText('10%\r20%\r')).toBe('20%');
  expect(commandOutputPanel(0,true)).toContain('Waiting for console output');
  expect(commandOutputPanel(0,false)).toContain('unavailable');
});
