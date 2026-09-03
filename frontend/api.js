const ACCESS_TOKEN_KEY = 'sabor-digital-access-token';
const LOCAL_ACCOUNT_KEY = 'sabor-digital-local-account';
const ACTIVE_USER_KEY = 'sabor-digital-active-user';
const CANTEEN_ACCOUNT = { id: 'canteen_admin', name: 'Cantina SESI', email: 'cantina@sabor-digital.local', role: 'canteen', studentId: '', className: 'Administração da cantina' };

export function getAccessToken() {
  return sessionStorage.getItem(ACCESS_TOKEN_KEY);
}

export function setAccessToken(token) {
  sessionStorage.setItem(ACCESS_TOKEN_KEY, token);
}

export function clearAccessToken() {
  sessionStorage.removeItem(ACCESS_TOKEN_KEY);
}

export function clearActiveUser() {
  sessionStorage.removeItem(ACTIVE_USER_KEY);
}

export async function hashCredential(value) {
  const bytes = new TextEncoder().encode(value);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

export function saveLocalAccount(account) {
  let accounts = [];
  try {
    const stored = JSON.parse(localStorage.getItem(LOCAL_ACCOUNT_KEY) || '[]');
    accounts = Array.isArray(stored) ? stored : stored ? [stored] : [];
  } catch { /* Start with an empty local account list when storage is invalid. */ }
  const index = accounts.findIndex((candidate) => candidate.email === account.email);
  if (index >= 0) accounts[index] = account;
  else accounts.push(account);
  localStorage.setItem(LOCAL_ACCOUNT_KEY, JSON.stringify(accounts));
}

export function getLocalAccount(email) {
  try {
    const stored = JSON.parse(localStorage.getItem(LOCAL_ACCOUNT_KEY) || '[]');
    const accounts = Array.isArray(stored) ? stored : stored ? [stored] : [];
    return email ? accounts.find((account) => account.email === email.trim().toLowerCase()) || null : accounts[0] || null;
  } catch { return null; }
}

export function getLocalAccounts() {
  try {
    const stored = JSON.parse(localStorage.getItem(LOCAL_ACCOUNT_KEY) || '[]');
    return Array.isArray(stored) ? stored : stored ? [stored] : [];
  } catch { return []; }
}

export function saveActiveUser(user) {
  sessionStorage.setItem(ACTIVE_USER_KEY, JSON.stringify(user));
}

export function getActiveUser() {
  try { return JSON.parse(sessionStorage.getItem(ACTIVE_USER_KEY) || 'null'); } catch { return null; }
}

export function isLocalSession() {
  return getAccessToken()?.startsWith('local:') || false;
}

export function isCanteenUser(user) {
  return user?.role === 'canteen' || user?.email === CANTEEN_ACCOUNT.email;
}

export async function authenticateLocalUser(email, password) {
  let account = getLocalAccount(email);
  if (!account && email.trim().toLowerCase() === CANTEEN_ACCOUNT.email && password === 'CantinaDemo!2026') {
    account = { ...CANTEEN_ACCOUNT, passwordHash: await hashCredential(password) };
  }
  if (!account) return null;
  const passwordHash = await hashCredential(password);
  if (passwordHash !== account.passwordHash) return null;
  return {
    id: account.id,
    name: account.name,
    email: account.email,
    role: account.role || 'student',
    status: 'active',
    studentId: account.studentId,
    className: account.className,
  };
}

export async function apiRequest(path, options = {}) {
  const headers = new Headers(options.headers || {});
  const token = getAccessToken();
  if (token) headers.set('Authorization', `Bearer ${token}`);
  if (options.body && !headers.has('Content-Type')) headers.set('Content-Type', 'application/json');

  const response = await fetch(path, { ...options, headers });
  if (response.status === 401) clearAccessToken();
  return response;
}

export async function apiErrorMessage(response, fallback = 'Não foi possível concluir a operação.') {
  try {
    const payload = await response.json();
    return payload?.error?.message || payload?.message || fallback;
  } catch {
    return fallback;
  }
}
