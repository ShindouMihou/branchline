/** Per-repository snapshots. Invalidated in-flight reads can never repopulate a cache. */
export function createBranchCache<T>(load:(root:string)=>Promise<T>){
  type Entry={value?:T;pending?:Promise<T>};const entries=new Map<string,Entry>();
  return {
    peek:(root:string)=>entries.get(root)?.value,
    loading:(root:string)=>!!entries.get(root)?.pending,
    invalidate:(root:string)=>{const value=entries.get(root)?.value;entries.set(root,{value})},
    refresh(root:string):Promise<T>{
      const existing=entries.get(root);if(existing?.pending)return existing.pending;
      const entry=existing??{};entries.delete(root);entries.set(root,entry);
      if(entries.size>8)entries.delete(entries.keys().next().value!);
      const pending=load(root).then(value=>{if(entries.get(root)===entry)entry.value=value;return value}).finally(()=>{if(entry.pending===pending)entry.pending=undefined});
      entry.pending=pending;return pending;
    }
  };
}
export function menuPosition(point:{x:number;y:number;alignRight?:boolean},size:{width:number;height:number},viewport:{width:number;height:number}){
  return {left:Math.max(8,Math.min(point.x-(point.alignRight?size.width:0),viewport.width-size.width-8)),top:Math.max(8,Math.min(point.y,viewport.height-size.height-8))};
}
