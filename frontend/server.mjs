import http from 'node:http';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readFile } from 'node:fs/promises';

const __filename = fileURLToPath(import.meta.url);
const frontendRoot = path.dirname(__filename);
const publicRoot = path.join(frontendRoot, 'public');
const port = Number(process.env.PORT || 4173);
const backendUrl = new URL(process.env.CANTINA_API_URL || 'http://127.0.0.1:8080');

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
  const appRoutes = new Set(['/cardapio', '/carrinho', '/meus-pedidos', '/login', '/cadastro']);
  const relativePath = pathname === '/' || appRoutes.has(pathname) ? 'index.html' : pathname.slice(1);
  const root = pathname.startsWith('/assets/') ? publicRoot : frontendRoot;
  const requestedPath = relativePath;
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
    'Cache-Control': ['.html', '.css', '.js'].includes(extension) ? 'no-cache' : 'public, max-age=3600',
  });
  response.end(body);
}

function isApiRequest(requestUrl) {
  return new URL(requestUrl, 'http://localhost').pathname.startsWith('/api/');
}

async function readRequestBody(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return Buffer.concat(chunks);
}

async function proxyApiRequest(request, response) {
  const incomingUrl = new URL(request.url || '/', 'http://localhost');
  const targetUrl = new URL(`${incomingUrl.pathname}${incomingUrl.search}`, backendUrl);
  const headers = new Headers();

  for (const headerName of ['accept', 'authorization', 'content-type', 'x-request-id']) {
    const value = request.headers[headerName];
    if (value) headers.set(headerName, Array.isArray(value) ? value.join(', ') : value);
  }

  const hasBody = !['GET', 'HEAD'].includes(request.method || 'GET');
  const apiResponse = await fetch(targetUrl, {
    method: request.method,
    headers,
    body: hasBody ? await readRequestBody(request) : undefined,
  });
  const responseHeaders = {};
  apiResponse.headers.forEach((value, key) => {
    if (!['connection', 'transfer-encoding'].includes(key)) responseHeaders[key] = value;
  });
  response.writeHead(apiResponse.status, responseHeaders);
  if (request.method === 'HEAD') {
    response.end();
    return;
  }
  response.end(Buffer.from(await apiResponse.arrayBuffer()));
}

const server = http.createServer(async (request, response) => {
  if (!['GET', 'HEAD', 'POST', 'PUT', 'PATCH', 'DELETE', 'OPTIONS'].includes(request.method)) {
    response.writeHead(405, { Allow: 'GET, HEAD, POST, PUT, PATCH, DELETE, OPTIONS' });
    response.end('Method Not Allowed');
    return;
  }

  if (isApiRequest(request.url || '/')) {
    try {
      await proxyApiRequest(request, response);
    } catch (error) {
      console.error(`API proxy failed for ${request.url}:`, error.message);
      response.writeHead(502, { 'Content-Type': 'application/json; charset=utf-8' });
      response.end(JSON.stringify({ error: 'Backend API unavailable' }));
    }
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
