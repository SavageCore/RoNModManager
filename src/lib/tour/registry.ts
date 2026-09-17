import type { TourActionName } from "./types";

type Handler = () => unknown;

const actions = new Map<string, Handler>();

/// A page registers its actions in onMount, which can be a moment after the
/// tour has navigated to it. Give it long enough to arrive rather than silently
/// doing nothing.
const WAIT_FOR_HANDLER_MS = 2000;
const POLL_MS = 50;

/// Pages register the specific UI operations the tour drives, so the engine
/// never reaches into page internals. Returns an unregister function.
export function registerTourActions(
  namespace: string,
  handlers: Record<string, Handler>,
): () => void {
  for (const [key, fn] of Object.entries(handlers)) {
    actions.set(`${namespace}:${key}`, fn);
  }
  return () => {
    for (const key of Object.keys(handlers)) {
      actions.delete(`${namespace}:${key}`);
    }
  };
}

function waitForHandler(
  name: string,
  timeoutMs: number,
): Promise<Handler | undefined> {
  return new Promise((resolve) => {
    const deadline = Date.now() + timeoutMs;
    const poll = () => {
      const handler = actions.get(name);
      if (handler) {
        resolve(handler);
        return;
      }
      if (Date.now() >= deadline) {
        resolve(undefined);
        return;
      }
      setTimeout(poll, POLL_MS);
    };
    poll();
  });
}

/// No-op when nothing ever registers the action: the engine navigates first and
/// waits for the step's target selector, which is the real sync point.
export async function callTourAction(name: TourActionName): Promise<void> {
  const handler =
    actions.get(name) ?? (await waitForHandler(name, WAIT_FOR_HANDLER_MS));
  await handler?.();
}
