export interface Appearance {
  fontSize: number;
  codeSize: number;
  spacing: 'compact' | 'comfortable' | 'relaxed';
  contrast: number;
  simplified: boolean;
}
export const appearanceKey = 'branchline.appearance.v1';
export const defaultAppearance: Appearance = { fontSize: 14, codeSize: 14, spacing: 'comfortable', contrast: 35, simplified: true };
export function parseAppearance(raw: string | null): Appearance {
  let input: Record<string, unknown> = {};
  try { const value = JSON.parse(raw ?? '{}'); if(value && typeof value === 'object' && !Array.isArray(value)) input = value; } catch { /* Use defaults for malformed local settings. */ }
  const bounded = (key: string, min: number, max: number, fallback: number) => typeof input[key] === 'number' && Number.isFinite(input[key]) ? Math.round(Math.max(min, Math.min(max, input[key] as number))) : fallback;
  return { fontSize: bounded('fontSize', 12, 18, 14), codeSize: bounded('codeSize', 12, 20, 14), contrast: bounded('contrast', 0, 100, 35), spacing: ['compact','comfortable','relaxed'].includes(String(input.spacing)) ? input.spacing as Appearance['spacing'] : 'comfortable', simplified: typeof input.simplified === 'boolean' ? input.simplified : true };
}
export const rowHeight = (value: Appearance) => ({ compact: 32, comfortable: 38, relaxed: 44 })[value.spacing];
export function applyAppearance(value: Appearance, root = document.documentElement) {
  const t=value.contrast/100;
  const density=({compact:0,comfortable:3,relaxed:6})[value.spacing];
  const shade=(a:number,b:number,offset=0)=>Math.round(a+(b-a)*t+offset);
  const surface=(offset:number)=>`rgb(${shade(26,8,offset)}, ${shade(27,9,offset)}, ${shade(30,11,offset)})`;
  const vars: Record<string,string> = {
    '--ui-font': `${value.fontSize}px`, '--code-font': `${value.codeSize}px`, '--code-line': `${Math.round(value.codeSize*1.65+density)}px`,
    '--log-row': `${rowHeight(value)}px`, '--density': `${density}px`,
    '--base': surface(0), '--panel':surface(4), '--raised':surface(9), '--control':surface(6), '--selected':surface(17),
    '--border':surface(25), '--text':`rgb(${shade(212,235)}, ${shade(214,236)}, ${shade(218,240)})`,
    '--muted':`rgb(${shade(166,177)}, ${shade(168,179)}, ${shade(174,185)})`,
    '--primary-bg':`rgb(${shade(199,224)}, ${shade(201,226)}, ${shade(206,232)})`,
  };
  for(const [key,css] of Object.entries(vars)) root.style.setProperty(key,css);
  root.dataset.spacing=value.spacing;
  root.dataset.simplified=String(value.simplified);
}
