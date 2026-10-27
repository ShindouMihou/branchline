import {it,expect} from 'vitest';
import {connectionGuide,shellCommand} from './connection-guide';
const configuration=JSON.stringify({mcpServers:{branchline:{command:'/Applications/Branchline App.app/Contents/MacOS/branchline',args:['--mcp','--credential-file',"/private/odd ' path/client.json"]}}});
it('generates the documented Claude and Codex commands with intact argument boundaries',()=>{
  const global=connectionGuide('claude-code',configuration,'global');expect(global.code).toContain('--scope user branchline --');expect(global.code).toContain("'/Applications/Branchline App.app/Contents/MacOS/branchline'");
  expect(connectionGuide('claude-code',configuration,'project').code).toContain('--scope local');
  expect(connectionGuide('codex',configuration,'global').code).toMatch(/^codex mcp add branchline -- /);
  const local=connectionGuide('codex',configuration,'project');expect(local.location).toBe('.codex/config.toml');expect(local.code).toContain('[mcp_servers.branchline]');expect(local.code).not.toContain('--scope');
  expect(shellCommand(['a','$(private)'])).toBe("a '$(private)'");
});
it('uses each client configuration location and preserves unrelated settings through merge instructions',()=>{
  expect(connectionGuide('cursor',configuration,'global').location).toBe('~/.cursor/mcp.json');
  const project=connectionGuide('cursor',configuration,'project');expect(project.location).toBe('.cursor/mcp.json');expect(JSON.parse(project.code).mcpServers.branchline.type).toBe('stdio');
  const desktop=connectionGuide('claude-desktop',configuration,'global');expect(desktop.after).toContain('keep other entries');expect(JSON.parse(desktop.code).mcpServers.branchline.command).toContain('Branchline App');
});

it('provides ChatGPT desktop setup in app settings without a CLI command',()=>{
  const guide=connectionGuide('chatgpt-desktop',configuration,'global');
  expect(guide.title).toContain('Settings → MCP servers → Add server');
  expect(guide.location).toContain('STDIO');
  expect(guide.code).toContain('/Applications/Branchline App.app/Contents/MacOS/branchline');
  expect(guide.code).toContain("/private/odd ' path/client.json");
  expect(guide.code).not.toContain('codex mcp add');
  expect(guide.after).toContain('Restart');
});
