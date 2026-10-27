import MarkdownIt from 'markdown-it';
import {escapeHtml} from './html';

const parser = new MarkdownIt({html:false,linkify:false,typographer:false});
parser.validateLink = url => {
  try {
    const parsed = new URL(url);
    return ['https:','http:'].includes(parsed.protocol) && !parsed.username && !parsed.password;
  } catch {
    return false;
  }
};
parser.renderer.rules.image = (tokens,index) => `<span class="markdown-image-label">${escapeHtml(tokens[index].content || 'Image omitted')}</span>`;
parser.renderer.rules.link_open = (tokens,index) => {
  const href = tokens[index].attrGet('href') ?? '';
  return `<a href="${escapeHtml(href)}" data-markdown-url="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer">`;
};

export function markdown(text:string) {
  return `<div class="markdown-content">${parser.render(text)}</div>`;
}
