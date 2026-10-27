import type {AiCommandDetail} from './ai-command-details';

export interface AiReviewDiff {
  path:string;
  source:'index'|'worktree';
  baseline?:'index'|'head';
  patch:string;
  snapshotId:string;
  added:number;
  removed:number;
  notice?:string;
  before?:string|null;
  after?:string|null;
}
export interface AiDiffSelection {
  planId:string;
  detailIndex:number;
  detail:AiCommandDetail;
  path:string;
  requestTitle:string;
}
