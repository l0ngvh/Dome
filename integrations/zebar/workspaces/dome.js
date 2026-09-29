import * as zebar from 'https://esm.sh/zebar@3.0';

let refresh = () => {};

export function watchWorkspaces(onChange) {
  refresh = async () => {
    const [rows, error] = await readWorkspaces();
    onChange(rows, error);
  };
  refresh();
  const timer = setInterval(refresh, 1000);
  return () => clearInterval(timer);
}

export async function focusWorkspace(name, monitor) {
  await zebar.shellExec('dome', ['focus-workspace', name, '--monitor', monitor]);
  refresh();
}

async function readWorkspaces() {
  try {
    const monitor = await zebar.currentWidget().window.tauri.currentMonitor();
    const res = await zebar.shellExec('dome', ['query', 'workspaces', '--monitor', monitor?.name ?? '']);
    if (res.exitCode != null && res.exitCode !== 0) {
      return [[], res.stderr.split(/\r?\n/)[0] || `dome exited with code ${res.exitCode}`];
    }
    return [JSON.parse(res.stdout), null];
  } catch (err) {
    return [[], String(err)];
  }
}
