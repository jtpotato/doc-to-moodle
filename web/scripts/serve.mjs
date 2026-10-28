import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../out/", import.meta.url));
const mime = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png" };
const server = createServer(async (request, response) => {
  try {
    const pathname = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
    let path = normalize(join(root, pathname));
    if (!path.startsWith(root)) throw new Error("Invalid path");
    if ((await stat(path)).isDirectory()) path = join(path, "index.html");
    response.setHeader("Content-Type", mime[extname(path)] || "application/octet-stream");
    response.setHeader("X-Content-Type-Options", "nosniff");
    response.end(await readFile(path));
  } catch {
    response.statusCode = 404;
    response.end("Not found");
  }
});
const port = Number(process.env.PORT || 3000);
server.listen(port, "127.0.0.1", () => console.log(`Serving static site at http://localhost:${port}`));
