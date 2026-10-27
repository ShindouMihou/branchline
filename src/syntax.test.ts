import {it,expect} from 'vitest';
import {detectLanguage,languages} from './syntax';
import {highlightSource} from './syntax-engine';
it('detects common repository files with a broad explicit language fallback',()=>{
  for(const [path,lang] of [['lib/main.dart','dart'],['src/app.tsx','tsx'],['src/main.rs','rust'],['assets/icon.svg','xml'],['Dockerfile','docker'],['pubspec.yaml','yaml']])expect(detectLanguage(path)).toBe(lang);
  expect(detectLanguage('unknown.extension')).toBeNull();expect(languages.length).toBeGreaterThan(200);
});
it('highlights complete source safely and keeps text unchanged',async()=>{
  const source='const message = "<img src=x onerror=alert(1)>";\n';
  const lines=await highlightSource(source,'typescript');expect(lines.map(line=>line.text).join('\n')).toBe(source);expect(lines.map(line=>line.html).join('')).toContain('data-token-color="');expect(lines.map(line=>line.html).join('')).not.toContain('<img');
  const dart=await highlightSource('class Welcome { final String message = "Hello"; }','dart');expect(dart[0].text).toContain('class Welcome');expect(dart[0].html).toContain('<span');
  expect(await highlightSource('x'.repeat(250001),'typescript')).toEqual([]);
});
