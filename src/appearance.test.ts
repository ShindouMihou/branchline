import { it, expect } from 'vitest';
import { parseAppearance, defaultAppearance, rowHeight } from './appearance';
it('loads defaults for corrupt or incompatible settings',()=>{
  for(const raw of [null,'{','null','[]','"value"']) expect(parseAppearance(raw)).toEqual(defaultAppearance);
});
it('restores saved preferences and bounds values before styling',()=>{
  const value=parseAppearance(JSON.stringify({fontSize:99,codeSize:4,contrast:-4,spacing:'relaxed',simplified:false}));
  expect(value).toEqual({fontSize:18,codeSize:12,contrast:0,spacing:'relaxed',simplified:false});
  expect(rowHeight(value)).toBe(44);
  expect(parseAppearance(JSON.stringify({...defaultAppearance,fontSize:'18',spacing:'unknown'}))).toEqual(defaultAppearance);
});
