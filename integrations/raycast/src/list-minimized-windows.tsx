import { execFile, execFileSync } from "node:child_process";
import { promisify } from "node:util";
import { useRef } from "react";
import {
  Action,
  ActionPanel,
  closeMainWindow,
  Detail,
  Icon,
  List,
  PopToRootType,
  Toast,
  showToast,
} from "@raycast/api";
import { showFailureToast, useCachedPromise } from "@raycast/utils";

type MinimizedWindow = {
  id: number;
  title: string;
  app_name: string | null;
  bundle_id: string | null;
  executable_path: string | null;
};

const DOME_BINARY = "dome";

// launchd PATH omits /opt/homebrew/bin on Apple Silicon.
const SPAWN_ENV = {
  ...process.env,
  PATH: ["/opt/homebrew/bin", "/usr/local/bin", process.env.PATH ?? ""]
    .filter((segment) => segment.length > 0)
    .join(":"),
};

const execFileAsync = promisify(execFile);

async function fetchMinimizedWindows(
  signal?: AbortSignal,
): Promise<MinimizedWindow[]> {
  const { stdout } = await execFileAsync(DOME_BINARY, ["query", "minimized"], {
    env: SPAWN_ENV,
    signal,
  });
  return JSON.parse(stdout) as MinimizedWindow[];
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
    await closeMainWindow({
      clearRootSearch: true,
      popToRootType: PopToRootType.Immediate,
    });
  } catch (err) {
    await showToast({
      style: Toast.Style.Failure,
      title: "Dome is not running",
      message: describeError(err),
    });
  }
}

export default function Command() {
  // Raycast may preserve the React tree on close, so rely on SWR plus a
  // manual Cmd-R revalidate rather than mount-only refetch.
  const abortable = useRef<AbortController | null>(null);
  const { isLoading, data, error, revalidate } = useCachedPromise(
    async () => fetchMinimizedWindows(abortable.current?.signal),
    [],
    {
      abortable,
      // ENOENT is handled by MissingBinaryView. Skip the toast.
      onError: async (err) => {
        if (isEnoent(err)) {
          return;
        }
        await showFailureToast(err, {
          title: "Dome is not running",
          primaryAction: {
            title: "Refresh",
            onAction: (toast) => {
              toast.hide();
              revalidate();
            },
          },
        });
      },
    },
  );

  if (error && isEnoent(error)) {
    return <MissingBinaryView />;
  }

  return (
    <List isLoading={isLoading}>
      {(data ?? []).map((entry) => (
        <List.Item
          key={entry.id}
          title={entry.title || "Untitled"}
          subtitle={entry.app_name ?? undefined}
          icon={
            entry.bundle_id ? { fileIcon: entry.bundle_id } : Icon.AppWindow
          }
          actions={
            <ActionPanel>
              <Action
                title="Restore Window"
                onAction={() => restoreWindow(entry)}
              />
              <Action
                title="Refresh"
                icon={Icon.ArrowClockwise}
                shortcut={{ modifiers: ["cmd"], key: "r" }}
                onAction={() => revalidate()}
              />
            </ActionPanel>
          }
        />
      ))}
    </List>
  );
}
