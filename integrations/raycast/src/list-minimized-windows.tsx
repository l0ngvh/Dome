import { execFileSync } from "node:child_process";
import {
  Action,
  ActionPanel,
  closeMainWindow,
  Detail,
  Icon,
  List,
  Toast,
  showToast,
} from "@raycast/api";

type MinimizedWindow = {
  id: number;
  title: string;
  app_id: string | null;
  app_name: string | null;
};

const DOME_BINARY = "dome";

// launchd PATH omits /opt/homebrew/bin on Apple Silicon.
const SPAWN_ENV = {
  ...process.env,
  PATH: ["/opt/homebrew/bin", "/usr/local/bin", process.env.PATH ?? ""]
    .filter((segment) => segment.length > 0)
    .join(":"),
};

type FetchResult =
  | { kind: "ok"; entries: MinimizedWindow[] }
  | { kind: "missing-binary" }
  | { kind: "daemon-down"; message: string };

function fetchMinimizedWindows(): FetchResult {
  try {
    const out = execFileSync(DOME_BINARY, ["query", "minimized"], {
      encoding: "utf8",
      env: SPAWN_ENV,
    });
    const parsed = JSON.parse(out) as MinimizedWindow[];
    return { kind: "ok", entries: parsed };
  } catch (err) {
    if (isEnoent(err)) {
      return { kind: "missing-binary" };
    }
    return { kind: "daemon-down", message: describeError(err) };
  }
}

function isEnoent(err: unknown): boolean {
  return (
    typeof err === "object" &&
    err !== null &&
    (err as { code?: string }).code === "ENOENT"
  );
}

function describeError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}

function MissingBinaryView() {
  const markdown = [
    "# `dome` is not on `PATH`",
    "",
    "Raycast spawns `dome` via `execFile`, so a shell alias in your rc file is not enough.",
    "Symlink the binary into `/usr/local/bin`, or add its install directory to your global `PATH`.",
    "",
    "See the top-level Dome README for install instructions.",
  ].join("\n");
  return <Detail markdown={markdown} />;
}

async function restoreWindow(entry: MinimizedWindow) {
  try {
    execFileSync(DOME_BINARY, ["unminimize-window", String(entry.id)], {
      env: SPAWN_ENV,
    });
    await closeMainWindow({ clearRootSearch: true });
  } catch (err) {
    await showToast({
      style: Toast.Style.Failure,
      title: "Dome is not running",
      message: describeError(err),
    });
  }
}

export default function Command() {
  const result = fetchMinimizedWindows();

  if (result.kind === "missing-binary") {
    return <MissingBinaryView />;
  }

  if (result.kind === "daemon-down") {
    showToast({
      style: Toast.Style.Failure,
      title: "Dome is not running",
      message: result.message,
    });
    return <List />;
  }

  return (
    <List>
      {result.entries.map((entry) => (
        <List.Item
          key={entry.id}
          title={entry.title || "Untitled"}
          subtitle={entry.app_name ?? undefined}
          icon={entry.app_id ? { fileIcon: entry.app_id } : Icon.AppWindow}
          actions={
            <ActionPanel>
              <Action
                title="Restore Window"
                onAction={() => restoreWindow(entry)}
              />
            </ActionPanel>
          }
        />
      ))}
    </List>
  );
}
