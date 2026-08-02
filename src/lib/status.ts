import { api } from "./api";
import type { FullStatus } from "./types";

let statusRequest: Promise<FullStatus> | null = null;

export function getStatus(): Promise<FullStatus> {
  if (!statusRequest) {
    statusRequest = api.getStatus().finally(() => {
      statusRequest = null;
    });
  }
  return statusRequest;
}

export async function waitForStatus(
  predicate: (status: FullStatus) => boolean,
  onStatus: (status: FullStatus) => void,
  timeoutMs = 30_000,
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const status = await getStatus();
      onStatus(status);
      if (predicate(status)) return true;
    } catch {
      // Retry until the component starts or the timeout expires.
    }
    await new Promise((resolve) => window.setTimeout(resolve, 900));
  }
  return false;
}
