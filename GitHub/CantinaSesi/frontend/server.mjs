import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';

const __filename = fileURLToPath(import.meta.url);
const frontendRoot = path.dirname(__filename);
const publicRoot = path.join(frontendRoot, 'public');
const port = Number(process.env.PORT || 4173);

const contentTypes = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.webp': 'image/webp',
};

function resolveRequestPath(requestPath) {
  const pathname = decodeURIComponent(new URL(requestPath, 'http://localhost').pathname);
  const relativePath = pathname === '/' ? 'index.html' : pathname.slice(1);
  const root = pathname.startsWith('/assets/') ? publicRoot : frontendRoot;
  const requestedPath = pathname.startsWith('/assets/')
    ? relativePath.slice('assets/'.length)
    : relativePath;
  const resolvedPath = path.resolve(root, requestedPath);

  if (resolvedPath !== root && !resolvedPath.startsWith(`${root}${path.sep}`)) {
    return null;
  }

  return resolvedPath;
}

async function sendFile(response, filePath) {
  const body = await readFile(filePath);
  const extension = path.extname(filePath).toLowerCase();
  response.writeHead(200, {
    'Content-Type': contentTypes[extension] || 'application/octet-stream',
    'Cache-Control': extension === '.html' ? 'no-cache' : 'public, max-age=3600',
  });
  response.end(body);
}

const server = http.createServer(async (request, response) => {
  if (request.method !== 'GET' && request.method !== 'HEAD') {
    response.writeHead(405, { Allow: 'GET, HEAD' });
    response.end('Method Not Allowed');
    return;
  }

  let filePath;
  try {
    filePath = resolveRequestPath(request.url || '/');
  } catch {
    filePath = null;
  }

  if (!filePath) {
    response.writeHead(403);
    response.end('Forbidden');
    return;
  }

  try {
    if (request.method === 'HEAD') {
      const body = await readFile(filePath);
      response.writeHead(200, {
        'Content-Type': contentTypes[path.extname(filePath).toLowerCase()] || 'application/octet-stream',
        'Content-Length': body.byteLength,
      });
      response.end();
      return;
    }
    await sendFile(response, filePath);
  } catch (error) {
    const status = error.code === 'ENOENT' ? 404 : 500;
    response.writeHead(status, { 'Content-Type': 'text/plain; charset=utf-8' });
    response.end(status === 404 ? 'Not Found' : 'Internal Server Error');
  }
});

server.listen(port, '0.0.0.0', () => {
  console.log(`Sabor Digital frontend running at http://localhost:${port}`);
});
