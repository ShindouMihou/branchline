import claudeLogo from '../assets/providers/claude.svg?raw';
import codexLogo from '../assets/providers/codex.svg?raw';
import cursorLogo from '../assets/providers/cursor.svg?raw';
export type Provider='claude-code'|'claude-desktop'|'codex'|'chatgpt-desktop'|'cursor'|'other';
export const providers:{id:Provider;name:string;description:string;logo:string}[]=[
  {id:'claude-code',name:'Claude Code',description:'Terminal & IDE',logo:claudeLogo},
  {id:'chatgpt-desktop',name:'ChatGPT Desktop',description:'Set up in the app',logo:codexLogo},
  {id:'codex',name:'Codex',description:'App, CLI & IDE',logo:codexLogo},
  {id:'cursor',name:'Cursor',description:'AI code editor',logo:cursorLogo},
  {id:'claude-desktop',name:'Claude Desktop',description:'Desktop chat',logo:claudeLogo},
  {id:'other',name:'Another client',description:'Any local MCP client',logo:'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M8 3v4m8-4v4M6 7h12v5a6 6 0 0 1-12 0V7Zm6 11v3"/></svg>'},
];
export const providerInfo=(id?:string)=>providers.find(p=>p.id===id)??providers.at(-1)!;
export const shellCommand=(parts:string[])=>parts.map(a=>/^[A-Za-z0-9_./:=@,+-]+$/.test(a)?a:`'${a.replaceAll("'","'\\''")}'`).join(' ');
interface Server {command:string;args:string[]}
export function connectionGuide(provider:Provider,configuration:string,scope:'global'|'project'){
  const data=JSON.parse(configuration);const server=data.mcpServers.branchline as Server;
  const args=[server.command,...server.args];const json=JSON.stringify({mcpServers:{branchline:{...(provider==='cursor'?{type:'stdio'}:{}),...server}}},null,2);
  if(provider==='chatgpt-desktop')return {title:'Open Settings → MCP servers → Add server',location:'Name: Branchline · Transport: STDIO',code:`Command:\n${server.command}\n\nArguments (add each as a separate entry):\n${server.args.join('\n')}`,language:'text',after:'Save the server, then select Restart. Start a new local chat and type /mcp to check that Branchline is connected.',verify:'',docs:'https://learn.chatgpt.com/docs/extend/mcp'};
  if(provider==='claude-code')return {title:'Run this in your terminal',location:scope==='global'?'Available across your Claude Code projects':'Run from the project you want to use',code:shellCommand(['claude','mcp','add','--transport','stdio','--scope',scope==='global'?'user':'local','branchline','--',...args]),language:'sh',after:'Start a new Claude Code session, then run /mcp to check Branchline.',verify:'claude mcp get branchline',docs:'https://code.claude.com/docs/en/mcp'};
  if(provider==='codex'&&scope==='global')return {title:'Run this in your terminal',location:'Shared by Codex app, CLI and IDE extension',code:shellCommand(['codex','mcp','add','branchline','--',...args]),language:'sh',after:'Start a new Codex session. Run /mcp in the CLI to check the connection.',verify:'codex mcp list',docs:'https://learn.chatgpt.com/docs/extend/mcp?surface=cli'};
  if(provider==='codex')return {title:'Add this to your project configuration',location:'.codex/config.toml',code:`[mcp_servers.branchline]\ncommand = ${JSON.stringify(server.command)}\nargs = ${JSON.stringify(server.args)}`,language:'toml',after:'Codex loads project configuration for trusted projects. Start a new session after saving.',verify:'codex mcp list',docs:'https://learn.chatgpt.com/docs/extend/mcp?surface=cli'};
  if(provider==='cursor')return {title:'Merge this into your MCP settings',location:scope==='global'?'~/.cursor/mcp.json':'.cursor/mcp.json',code:json,language:'json',after:'In Cursor, open MCP settings and check that Branchline is enabled. Keep your other server entries.',verify:'',docs:'https://cursor.com/docs/mcp'};
  if(provider==='claude-desktop')return {title:'Add Branchline in Claude Desktop',location:'Settings → Developer → Edit Config',code:json,language:'json',after:'Merge this into claude_desktop_config.json, keep other entries, then completely quit and reopen Claude Desktop.',verify:'',docs:'https://modelcontextprotocol.io/docs/develop/connect-local-servers'};
  return {title:'Add a local MCP server in your client',location:'Your client’s MCP server settings',code:json,language:'json',after:'Merge this with your existing settings, then restart the client or reconnect its MCP servers.',verify:'',docs:'https://modelcontextprotocol.io/docs/develop/connect-local-servers'};
}

export function desktopSetupFields(configuration:string){
  const server=JSON.parse(configuration).mcpServers.branchline as Server;
  return [{label:'Name',value:'Branchline'},{label:'Transport',value:'STDIO'},{label:'Command to launch',value:server.command},...server.args.map((value,index)=>({label:`Argument ${index+1}`,value}))];
}
