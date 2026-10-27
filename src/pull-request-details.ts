import type {AiCommandDetail} from './types/ai-command-details';

type Field=(label:string,value?:string,prominent?:boolean)=>string;
type MarkdownField=(label:string,value?:string)=>string;
type Route=(label:string,source?:string,target?:string)=>string;

export function pullRequestContent(detail:AiCommandDetail,field:Field,markdownField:MarkdownField,route:Route):string|null {
 const edit=detail.kind==='pr.edit';
 if(!edit&&detail.kind!=='pr.create')return null;
 const target=detail.pullRequestTarget;
 const identity=edit?`${field('Pull request',target?/^\d+$/.test(target)?`#${target}`:target:'For the current branch at execution')}${detail.repository?field('GitHub repository',detail.repository):''}`:detail.repository?field('GitHub repository',detail.repository):'';
 const title=edit?detail.pullRequestTitle!==undefined?field('New title',detail.pullRequestTitle,true):'':field('Pull request title',detail.pullRequestTitle,true);
 const branches=edit?detail.baseBranch!==undefined?field('New base branch',detail.baseBranch):'':route('Source → target',detail.headBranch,detail.baseBranch);
 const description=edit&&detail.body===undefined&&!detail.bodySource?'':markdownField(edit?'New description':'Description',detail.body);
 return `${identity}${title}${branches}${description}`;
}
