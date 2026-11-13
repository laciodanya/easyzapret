import { describe, expect, it } from "vitest";
import { createSerialQueue } from "./serialQueue";

describe("createSerialQueue", () => {
  it("keeps settings writes in submission order", async () => {
    const enqueue = createSerialQueue();
    const events: string[] = [];
    let releaseFirst!: () => void;
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });

    const first = enqueue(async () => {
      events.push("first:start");
      await firstGate;
      events.push("first:end");
      return 1;
    });
    const second = enqueue(async () => {
      events.push("second");
      return 2;
    });

    await Promise.resolve();
    expect(events).toEqual(["first:start"]);
    releaseFirst();
    await expect(Promise.all([first, second])).resolves.toEqual([1, 2]);
    expect(events).toEqual(["first:start", "first:end", "second"]);
  });

  it("continues after a failed write", async () => {
    const enqueue = createSerialQueue();
    const failed = enqueue(async () => {
      throw new Error("disk full");
    });
    const next = enqueue(async () => "saved");

    await expect(failed).rejects.toThrow("disk full");
    await expect(next).resolves.toBe("saved");
  });
});
