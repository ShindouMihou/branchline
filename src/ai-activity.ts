const clearable=new Set(['succeeded','failed','partially_completed','cancelled','blocked','needs_review','outcome_unknown']);
export const canClearActivity=(status:string)=>clearable.has(status);
export const matchesInbox=(status:string,filter:'waiting'|'activity')=>filter==='waiting'?status==='pending_review':status!=='pending_review';
