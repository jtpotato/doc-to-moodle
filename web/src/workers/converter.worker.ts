/// <reference lib="webworker" />
declare const self: DedicatedWorkerGlobalScope;

type WasmModule = {
  default: (input?: string | { module_or_path?: string }) => Promise<unknown>;
  convert_docx_preview: (bytes: Uint8Array) => string;
};

self.onmessage = async (event: MessageEvent<ArrayBuffer>) => {
  try {
    const wasm = await (new Function("path", "return import(path)")(
      "/doc-to-moodle/wasm/converter.js",
    ) as Promise<WasmModule>);
    await wasm.default("/doc-to-moodle/wasm/converter_bg.wasm");
    const result = JSON.parse(
      wasm.convert_docx_preview(new Uint8Array(event.data)),
    );
    self.postMessage({ ok: true, result });
  } catch (error) {
    self.postMessage({
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};

export {};
