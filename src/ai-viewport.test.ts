// @vitest-environment happy-dom
import {it,expect} from 'vitest';
import {captureAiViewport,restoreAiViewport} from './ai-viewport';
const markup=(view='request-a')=>`<section class="ai-modal" data-ai-view="${view}"><div class="ai-inbox-list"></div><div class="ai-review-scroll"><details data-ai-disclosure="note"><summary>AI note</summary><p>Long explanation</p></details><button data-ai-step="1">Manual step</button></div></section>`;
it('restores expanded content before the browser clamps the scroll offset, including keyboard focus',()=>{
  document.body.innerHTML=markup();document.querySelector('details')!.open=true;
  document.querySelector<HTMLElement>('[data-ai-step]')!.focus();
  document.querySelector('.ai-review-scroll')!.scrollTop=420;document.querySelector('.ai-inbox-list')!.scrollTop=80;
  const state=captureAiViewport(document);
  document.body.innerHTML=markup();
  const review=document.querySelector('.ai-review-scroll')!;let actualScroll=0;
  Object.defineProperty(review,'scrollTop',{get:()=>actualScroll,set:(value:number)=>{actualScroll=Math.min(value,document.querySelector('details')!.open?800:0)}});
  restoreAiViewport(document,state);
  expect(document.querySelector('details')!.open).toBe(true);expect(review.scrollTop).toBe(420);expect(document.querySelector('.ai-inbox-list')!.scrollTop).toBe(80);expect((document.activeElement as HTMLElement).dataset.aiStep).toBe('1');
});
it('does not carry another request’s scroll or disclosures into a new view',()=>{
  document.body.innerHTML=markup();document.querySelector('details')!.open=true;document.querySelector('.ai-review-scroll')!.scrollTop=300;
  const state=captureAiViewport(document);document.body.innerHTML=markup('request-b');restoreAiViewport(document,state);
  expect(document.querySelector('details')!.open).toBe(false);expect(document.querySelector('.ai-review-scroll')!.scrollTop).toBe(0);
});
