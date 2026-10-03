// Tiny static server for testing the web build locally: node web/serve.cjs [dir] [port]
const http = require("http"), fs = require("fs"), path = require("path");
const root = path.resolve(process.argv[2] || "docs"), port = +(process.argv[3] || 8123);
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".png": "image/png", ".json": "application/json" };
http.createServer((req, res) => {
    let p = path.join(root, decodeURIComponent(req.url.split("?")[0]));
    if (!p.startsWith(root)) { res.writeHead(403); return res.end(); }
    if (fs.existsSync(p) && fs.statSync(p).isDirectory()) p = path.join(p, "index.html");
    fs.readFile(p, (err, data) => {
        if (err) { res.writeHead(404); return res.end("not found"); }
        res.writeHead(200, { "Content-Type": types[path.extname(p)] || "application/octet-stream", "Cache-Control": "no-store" });
        res.end(data);
    });
}).listen(port, "127.0.0.1", () => console.log("serving " + root + " on http://localhost:" + port));
