/**
 * Runs asynchronous writes in submission order. A rejected write does not
 * poison the queue, so a newer settings change can still be persisted.
 */
export function createSerialQueue() {
  let tail: Promise<unknown> = Promise.resolve();

  return function enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const current = tail.then(operation);
    tail = current.catch(() => {});
    return current;
  };
}
