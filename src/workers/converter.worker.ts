/// <reference lib="webworker" />
import { convertDocx } from "@/lib/converter";

declare const self: DedicatedWorkerGlobalScope;

self.onmessage = async (event: MessageEvent<ArrayBuffer>) => {
  try {
    const result = await convertDocx(event.data);
    self.postMessage({ ok: true, result });
  } catch (error) {
    self.postMessage({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};

export {};
