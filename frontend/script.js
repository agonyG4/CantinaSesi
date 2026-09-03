import { createNavbar } from './navbar.js';
import { apiErrorMessage, apiRequest, clearAccessToken, clearActiveUser, getAccessToken, getActiveUser, getLocalAccounts, isCanteenUser, isLocalSession } from './api.js';
import { initLoginModal, openLoginModal } from './login-modal.js';
import { initRegisterModal, openRegisterModal } from './register-modal.js';
import { initProfileModal, openProfileModal } from './profile-modal.js';
import { initFaqModal } from './faq-modal.js';

const CART_KEY = 'sabor-digital-cart';
const PAYMENT_KEY = 'sabor-digital-payment';
const CURRENT_ORDER_KEY = 'sabor-digital-current-order';
const ORDERS_KEY = 'sabor-digital-orders';
const PROFILE_KEY = 'sabor-digital-profile';
const CATALOG_OVERRIDES_KEY = 'sabor-digital-catalog-overrides';
let orderRefreshTimer;

const demoProfile = { name: 'Lucas Almeida', email: 'lucas@example.com', studentId: '20251234', className: '3º ano • Desenvolvimento de Sistemas' };
const demoOrder = { id: '67', status: 'preparing', total: 2750, items: [{ product_name: 'Hamburgão duplo', quantity: 2 }, { product_name: 'Suco de laranja', quantity: 1 }] };

const fallbackProducts = [
  { id: 'hamburgao-duplo', name: 'Hamburgão duplo', description: 'Pão, carne, molho e bacon.', price: 900, category: 'Lanche', categoryId: 'snacks', image: '/assets/buns.png.b64', tone: 'cream' },
  { id: 'pao-de-queijo', name: 'Pão de queijo', description: 'Porção quentinha para o intervalo.', price: 450, category: 'Rápido', categoryId: 'snacks', image: '/assets/pao-de-queijo.png.b64', tone: 'yellow' },
  { id: 'coxinha', name: 'Coxinha', description: 'Coxinha de frango com recheio cremoso.', price: 900, category: 'Lanche', categoryId: 'snacks', image: '/assets/buns.png.b64', tone: 'peach' },
  { id: 'suco-laranja', name: 'Suco de laranja', description: '300ml', price: 400, category: 'Bebida', categoryId: 'drinks', image: '/assets/suco-laranja.png.b64', tone: 'cream' },
  { id: 'agua-mineral', name: 'Água mineral', description: 'Garrafa de 500 ml sem gás.', price: 400, category: 'Bebida', categoryId: 'drinks', image: '/assets/suco-laranja.png.b64', tone: 'blue' },
  { id: 'brownie', name: 'Brownie', description: 'Brownie de chocolate com casquinha.', price: 600, category: 'Doce', categoryId: 'sweets', image: '/assets/buns.png.b64', tone: 'brown' },
  { id: 'sanduiche-natural', name: 'Sanduíche natural', description: 'Pão, presunto e queijo, tomate e salada.', price: 600, category: 'Saudável', categoryId: 'healthy', image: '/assets/buns.png.b64', tone: 'cream' },
  { id: 'salgado-assado', name: 'Salgado assado', description: 'Assado quentinho para o intervalo.', price: 700, category: 'Indisponível', categoryId: 'snacks', image: '/assets/buns.png.b64', tone: 'green', available: false },
];

const money = (cents) => new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'BRL' }).format(cents / 100);

function readCart() {
  try { return JSON.parse(localStorage.getItem(CART_KEY) || '[]'); } catch { return []; }
}

function saveCart(cart) {
  localStorage.setItem(CART_KEY, JSON.stringify(cart));
  updateCartCount();
}

function readJson(key, fallback) {
  try { return JSON.parse(localStorage.getItem(key) || 'null') || fallback; } catch { return fallback; }
}

function escapeHtml(value = '') {
  return String(value).replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[character]));
}

function saveCurrentOrder(order) {
  const savedOrder = { ...order, source: 'checkout', savedAt: new Date().toISOString() };
  localStorage.setItem(CURRENT_ORDER_KEY, JSON.stringify(savedOrder));
  const orders = readJson(ORDERS_KEY, []);
  const existing = Array.isArray(orders) ? orders.filter((item) => item.id !== savedOrder.id) : [];
  localStorage.setItem(ORDERS_KEY, JSON.stringify([savedOrder, ...existing]));
}

function readLocalOrders() {
  const orders = readJson(ORDERS_KEY, []);
  if (Array.isArray(orders) && orders.length) return orders;
  const current = readJson(CURRENT_ORDER_KEY, null);
  return current ? [current] : [];
}

function updateLocalOrderStatus(id, status) {
  const orders = readLocalOrders().map((order) => order.id === id ? { ...order, status, updatedAt: new Date().toISOString() } : order);
  localStorage.setItem(ORDERS_KEY, JSON.stringify(orders));
  const current = readJson(CURRENT_ORDER_KEY, null);
  if (current?.id === id) localStorage.setItem(CURRENT_ORDER_KEY, JSON.stringify({ ...current, status, updatedAt: new Date().toISOString() }));
}

function applyCatalogOverrides(products) {
  const overrides = readJson(CATALOG_OVERRIDES_KEY, {});
  return products.map((product) => ({ ...product, ...(overrides[product.id] || {}) }));
}

function saveCatalogProduct(product) {
  const overrides = readJson(CATALOG_OVERRIDES_KEY, {});
  overrides[product.id] = product;
  localStorage.setItem(CATALOG_OVERRIDES_KEY, JSON.stringify(overrides));
}

function createPickupCode() {
  const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
  const values = crypto.getRandomValues(new Uint8Array(6));
  return [...values].map((value) => alphabet[value % alphabet.length]).join('');
}

function createLocalOrder(cart) {
  const localSessionId = getAccessToken()?.startsWith('local:') ? getAccessToken().slice('local:'.length) : '';
  const user = getLocalAccounts().find((account) => account.id === localSessionId) || getActiveUser();
  return {
    id: `local-order-${Date.now()}`,
    student_id: user?.id || 'local_student',
    student_name: user?.name || 'Aluno',
    student_email: user?.email || '',
    status: 'pending',
    total: cart.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0),
    pickup_code: createPickupCode(),
    items: cart.map((item) => ({ product_id: item.productId, product_name: item.name, unit_price: item.unitPrice, quantity: item.quantity })),
  };
}

function orderStatusIndex(status) {
  return { pending: 0, confirmed: 1, preparing: 2, ready: 3, delivered: 4 }[status] ?? 0;
}

function orderStatusLabel(status) {
  return { pending: 'AGUARDANDO CONFIRMAÇÃO', confirmed: 'CONFIRMADO', preparing: 'EM PREPARO', ready: 'PRONTO PARA RETIRADA', delivered: 'ENTREGUE', cancelled: 'CANCELADO' }[status] || 'AGUARDANDO CONFIRMAÇÃO';
}

const orderStatuses = [
  ['pending', 'Aguardando confirmação'],
  ['confirmed', 'Pedido confirmado'],
  ['preparing', 'Em preparo'],
  ['ready', 'Pronto para retirada'],
  ['delivered', 'Entregue'],
  ['cancelled', 'Cancelado'],
];

function statusOptions(currentStatus) {
  return orderStatuses.map(([value, label]) => `<option value="${value}" ${value === currentStatus ? 'selected' : ''}>${label}</option>`).join('');
}

function orderIdLabel(id) { return String(id || '').startsWith('#') ? String(id) : `#${id}`; }

function orderItemsLabel(order) {
  return (order.items || []).map((item) => `${item.product_name || item.name || 'Item'}${item.quantity > 1 ? ` (${item.quantity}x)` : ''}`).join(' + ') || 'Itens do pedido';
}

async function loadProfile() {
  const accessToken = getAccessToken();
  const localSessionId = accessToken?.startsWith('local:') ? accessToken.slice('local:'.length) : '';
  const activeUser = accessToken ? (getActiveUser() || getLocalAccounts().find((account) => account.id === localSessionId) || null) : null;
  let profile = { ...demoProfile, ...(activeUser || {}) };
  if (accessToken && !isLocalSession()) {
    try {
      const response = await apiRequest('/api/v1/auth/me');
      if (response.ok) {
        const user = await response.json();
        profile = { ...profile, name: user.name || profile.name, email: user.email || profile.email };
      }
    } catch { /* The local profile remains available while the API is offline. */ }
  }
  const profileKey = `${PROFILE_KEY}:${profile.id || profile.email || 'default'}`;
  const stored = readJson(profileKey, activeUser ? {} : readJson(PROFILE_KEY, {}));
  return { ...profile, ...stored, profileKey };
}

async function loadTrackedOrder() {
  const stored = readJson(CURRENT_ORDER_KEY, null);
  if (!stored) return { ...demoOrder, isDemo: true };
  if (String(stored.id).startsWith('local-order-')) return stored;
  if (stored.source === 'checkout' && stored.id) {
    try {
      const response = await apiRequest(`/api/v1/orders/${encodeURIComponent(stored.id)}`);
      if (response.ok) {
        const latest = await response.json();
        const merged = { ...stored, ...latest, source: 'checkout' };
        localStorage.setItem(CURRENT_ORDER_KEY, JSON.stringify(merged));
        return merged;
      }
    } catch { /* Keep the last known status so tracking still works offline. */ }
  }
  return stored;
}

function cartCount() { return readCart().reduce((total, item) => total + item.quantity, 0); }

function updateCartCount() {
  document.querySelectorAll('[data-cart-count]').forEach((node) => { node.textContent = cartCount() || ''; });
}

function addToCart(product) {
  if (!product) return;
  if (!getAccessToken()) {
    openLoginModal();
    return false;
  }
  const cart = readCart();
  const existing = cart.find((item) => item.productId === product.id);
  if (existing) existing.quantity += 1;
  else cart.push({ productId: product.id, name: product.name, unitPrice: product.price, quantity: 1, description: product.description, image: product.image, tone: product.tone });
  saveCart(cart);
  return true;
}

function currentRoute() { return window.location.pathname.replace(/\/$/, '') || '/'; }

function pageShell(active, content, navOptions = {}) {
  document.body.classList.add('app-screen');
  const effectiveNavOptions = { ...navOptions, showLogout: navOptions.showLogout ?? Boolean(getAccessToken()) };
  document.body.innerHTML = `<div class="app-screen-shell">${createNavbar(active, effectiveNavOptions)}<main class="app-main">${content}</main></div>`;
  document.title = `Sabor Digital | ${active === 'menu' ? 'Cardápio' : active === 'cart' ? 'Carrinho' : active === 'orders' ? 'Meus pedidos' : ['canteen', 'canteen-menu', 'canteen-students'].includes(active) ? 'Painel da cantina' : 'Cantina SESI'}`;
  updateCartCount();
}

async function hydrateImages(scope = document) {
  await Promise.all([...scope.querySelectorAll('[data-asset]')].map(async (image) => {
    try {
      const response = await fetch(image.dataset.asset);
      if (response.ok) image.src = `data:image/png;base64,${(await response.text()).trim()}`;
    } catch { /* asset fallback stays blank */ }
  }));
}

async function loadProducts() {
  try {
    const [response, categoriesResponse] = await Promise.all([
      fetch('/api/v1/products'),
      fetch('/api/v1/categories'),
    ]);
    if (!response.ok) throw new Error('catalog unavailable');
    const payload = await response.json();
    if (!Array.isArray(payload.items) || !payload.items.length) throw new Error('empty catalog');
    const categoriesPayload = categoriesResponse.ok ? await categoriesResponse.json() : { items: [] };
    const categoryNames = new Map((categoriesPayload.items || []).map((category) => [category.id, category.name]));
    return applyCatalogOverrides(payload.items.map((item, index) => {
      const backendCategoryId = item.category_id || 'category_snacks';
      const categoryId = backendCategoryId.replace(/^category_/, '');
      return {
        ...item,
        category: item.category || categoryNames.get(backendCategoryId) || 'Lanche',
        categoryId,
        tone: fallbackProducts[index % fallbackProducts.length].tone,
        image: fallbackProducts[index % fallbackProducts.length].image,
      };
    }));
  } catch {
    try {
      const response = await fetch('/data/catalog.json');
      if (!response.ok) throw new Error('local catalog unavailable');
      const payload = await response.json();
      if (!Array.isArray(payload.products) || !payload.products.length) throw new Error('empty local catalog');
      return applyCatalogOverrides(payload.products);
    } catch { return applyCatalogOverrides(fallbackProducts); }
  }
}

function productCard(product, compact = false) {
  const disabled = product.available === false;
  return `<article class="catalog-product ${compact ? 'is-compact' : ''} ${disabled ? 'is-disabled' : ''}"><div class="catalog-product-image tone-${product.tone || 'cream'}"><img data-asset="${product.image || ''}" src="" alt="${product.name}" /></div><span class="category-badge badge-${product.categoryId || 'snacks'}">${product.category || 'Lanche'}</span><h3>${product.name}</h3><p>${product.description || ''}</p><div class="product-footer"><strong>${money(product.price)}</strong>${disabled ? '<span class="unavailable-label">Indisponível</span>' : `<button class="small-red-button" data-add-product="${product.id}">Adicionar</button>`}</div></article>`;
}

async function renderMenu() {
  pageShell('menu', `<section class="page-heading"><h1>Cardápio</h1><p>Escolha seus itens e monte o pedido antes do intervalo.</p></section><section class="catalog-controls"><label class="search-control"><span aria-hidden="true">⌕</span><input id="product-search" placeholder="Buscar produto..." /></label><div class="category-controls"><button class="filter-button is-selected" data-category="all">Todos</button><button class="filter-button is-red" data-category="snacks">Lanches</button><button class="filter-button is-yellow" data-category="drinks">Bebidas</button><button class="filter-button" data-category="bakery">Padaria</button><button class="filter-button" data-category="sweets">Doces</button><button class="filter-button" data-category="healthy">Saudáveis</button></div></section><section class="catalog-grid" id="catalog-grid" aria-live="polite"><p class="loading-note">Carregando cardápio...</p></section><section class="catalog-summary"><div><strong id="available-count">0 produtos disponíveis hoje</strong><small>Valores ilustrativos para o pedido.</small></div><a class="small-red-button" href="/carrinho">Ir para o carrinho</a></section>`);
  const products = await loadProducts();
  const grid = document.querySelector('#catalog-grid');
  const render = () => {
    const search = document.querySelector('#product-search').value.trim().toLowerCase();
    const category = document.querySelector('.filter-button.is-selected')?.dataset.category || 'all';
    const filtered = products.filter((item) => (category === 'all' || item.categoryId === category) && (!search || `${item.name} ${item.description}`.toLowerCase().includes(search)));
    grid.innerHTML = filtered.length ? filtered.map((item) => productCard(item)).join('') : '<p class="empty-note">Nenhum produto encontrado.</p>';
    document.querySelector('#available-count').textContent = `${products.filter((item) => item.available !== false).length} produtos disponíveis hoje`;
    hydrateImages(grid);
    grid.querySelectorAll('[data-add-product]').forEach((button) => button.addEventListener('click', () => { if (!addToCart(products.find((item) => item.id === button.dataset.addProduct))) return; button.textContent = 'Adicionado'; setTimeout(() => { button.textContent = 'Adicionar'; }, 900); }));
  };
  document.querySelector('#product-search').addEventListener('input', render);
  document.querySelectorAll('.filter-button').forEach((button) => button.addEventListener('click', () => { document.querySelectorAll('.filter-button').forEach((item) => item.classList.remove('is-selected')); button.classList.add('is-selected'); render(); }));
  render();
}

function cartItem(item) {
  return `<article class="cart-item"><div class="cart-thumb tone-${item.tone || 'cream'}"><img data-asset="${item.image || ''}" src="" alt="${item.name}" /></div><div class="cart-item-copy"><strong>${item.name}</strong><span>1 unidade</span><div class="quantity-control"><button data-decrease="${item.productId}" aria-label="Diminuir quantidade">−</button><span>${item.quantity}</span><button data-increase="${item.productId}" aria-label="Aumentar quantidade">+</button></div></div><div class="cart-item-price"><strong>${money(item.unitPrice * item.quantity)}</strong><button data-remove="${item.productId}">Remover</button></div></article>`;
}

async function renderCart() {
  pageShell('cart', `<section class="page-heading"><h1>Seu carrinho</h1><p>Revise os itens antes de confirmar o pedido.</p></section><section class="cart-layout"><div class="cart-items" id="cart-items"></div><aside class="order-summary"><h2>Resumo do pedido</h2><div class="summary-row"><span>Subtotal</span><strong id="cart-subtotal">R$ 0,00</strong></div><div class="summary-row"><span>Taxa</span><strong>R$ 0,00</strong></div><div class="summary-total"><span>Total</span><strong id="cart-total">R$ 0,00</strong></div><h3>Forma de pagamento</h3><label class="payment-option"><input type="radio" name="payment" value="pix" /> Pix</label><label class="payment-option"><input type="radio" name="payment" value="card" /> Cartão de crédito</label><button class="primary-action" id="confirm-order">Confirmar pedido</button><p class="form-feedback" id="cart-feedback"></p></aside></section><section class="pickup-banner"><div><h2>Retirada rápida</h2><p>Você receberá uma atualização quando o pedido estiver pronto. Mostre o número do pedido na cantina.</p></div><span>Tempo estimado: 10-15 min</span></section><section class="suggestions"><h2>Sugestões para adicionar</h2><div class="suggestion-row" id="suggestion-row"></div></section>`);
  const products = await loadProducts();
  const render = () => {
    const cart = readCart(); const items = document.querySelector('#cart-items');
    items.innerHTML = cart.length ? cart.map(cartItem).join('') : '<div class="empty-cart"><h2>Seu carrinho está vazio</h2><p>Escolha um lanche no cardápio para começar.</p><a class="small-red-button" href="/cardapio">Ver cardápio</a></div>';
    const subtotal = cart.reduce((sum, item) => sum + item.unitPrice * item.quantity, 0);
    document.querySelector('#cart-subtotal').textContent = money(subtotal); document.querySelector('#cart-total').textContent = money(subtotal); hydrateImages(items);
    items.querySelectorAll('[data-increase]').forEach((button) => button.addEventListener('click', () => changeQuantity(button.dataset.increase, 1)));
    items.querySelectorAll('[data-decrease]').forEach((button) => button.addEventListener('click', () => changeQuantity(button.dataset.decrease, -1)));
    items.querySelectorAll('[data-remove]').forEach((button) => button.addEventListener('click', () => { saveCart(readCart().filter((item) => item.productId !== button.dataset.remove)); render(); }));
  };
  const changeQuantity = (id, delta) => { const cart = readCart(); const item = cart.find((entry) => entry.productId === id); if (!item) return; item.quantity += delta; saveCart(item.quantity <= 0 ? cart.filter((entry) => entry.productId !== id) : cart); render(); };
  document.querySelector('#suggestion-row').innerHTML = products.filter((item) => !readCart().some((entry) => entry.productId === item.id)).slice(0, 3).map((item) => productCard({ ...item, available: true }, true)).join('');
  hydrateImages(document.querySelector('#suggestion-row'));
  document.querySelectorAll('#suggestion-row [data-add-product]').forEach((button) => button.addEventListener('click', () => { if (!addToCart(products.find((item) => item.id === button.dataset.addProduct))) return; render(); }));
  document.querySelectorAll('input[name="payment"]').forEach((input) => { input.checked = input.value === (localStorage.getItem(PAYMENT_KEY) || 'pix'); input.addEventListener('change', () => localStorage.setItem(PAYMENT_KEY, input.value)); });
  document.querySelector('#confirm-order').addEventListener('click', async () => {
    const feedback = document.querySelector('#cart-feedback');
    const cart = readCart();
    const payment = document.querySelector('input[name="payment"]:checked')?.value;
    if (!cart.length) { feedback.textContent = 'Adicione pelo menos um item ao carrinho.'; return; }
    if (!payment) { feedback.textContent = 'Escolha uma forma de pagamento.'; return; }
    if (!getAccessToken()) {
      feedback.textContent = 'Entre na sua conta para enviar o pedido à cantina.';
      openLoginModal();
      return;
    }

    const button = document.querySelector('#confirm-order');
    button.disabled = true;
    feedback.textContent = 'Enviando pedido para a cantina...';
    try {
      let order;
      if (isLocalSession()) {
        order = createLocalOrder(cart);
      } else {
        const response = await apiRequest('/api/v1/orders', {
          method: 'POST',
          body: JSON.stringify({ items: cart.map((item) => ({ product_id: item.productId, quantity: item.quantity })) }),
        });
        if (!response.ok) throw new Error(await apiErrorMessage(response, 'Não foi possível enviar o pedido.'));
        order = await response.json();
      }
      saveCurrentOrder(order);
      localStorage.removeItem(CART_KEY);
      render();
      document.querySelector('#cart-feedback').textContent = `${isLocalSession() ? 'Pedido registrado' : 'Pedido enviado para a cantina'}: ${orderIdLabel(order.id)}. Retire com o código ${order.pickup_code}. Pagamento: ${payment === 'pix' ? 'Pix' : 'cartão'}.`;
    } catch (error) {
      feedback.textContent = error.message || 'Não foi possível enviar o pedido.';
    } finally {
      button.disabled = false;
    }
  });
  render();
}

function trackingMarkup(order) {
  const currentIndex = orderStatusIndex(order.status);
  const labels = [order.status === 'pending' ? 'Aguardando confirmação' : 'Pedido confirmado', 'Em preparo', 'Pronto para retirada', 'Entregue'];
  const tracker = labels.map((label, index) => `<li class="${currentIndex > index ? 'done' : currentIndex === index ? 'current' : ''}"><span class="status-dot"></span>${label}</li>`).join('');
  const statusClass = order.status === 'cancelled' ? 'is-cancelled' : '';
  return `<article class="current-order-card ${statusClass}"><span class="order-status-pill">${orderStatusLabel(order.status)}</span><h3>${escapeHtml(orderIdLabel(order.id))}</h3><p>${order.items?.reduce((total, item) => total + (item.quantity || 0), 0) || 0} itens • ${money(order.total || 0)}</p><ol class="status-tracker">${tracker}</ol>${order.pickup_code ? `<div class="pickup-code"><span>Código de retirada</span><strong>${escapeHtml(order.pickup_code)}</strong></div>` : ''}${order.status === 'cancelled' ? '<p class="order-cancelled-note">Este pedido foi cancelado.</p>' : ''}</article>`;
}

function updateOrderTracking(order) {
  const card = document.querySelector('#tracked-order-card');
  if (card) card.outerHTML = trackingMarkup(order).replace('current-order-card', 'current-order-card" id="tracked-order-card');
  const updated = document.querySelector('#order-last-updated');
  if (updated) updated.textContent = `Última atualização: ${new Intl.DateTimeFormat('pt-BR', { hour: '2-digit', minute: '2-digit' }).format(new Date())}`;
}

async function refreshTrackedOrder(order) {
  if (!order?.id || order.isDemo || order.source !== 'checkout') return;
  if (String(order.id).startsWith('local-order-')) {
    const latest = readJson(CURRENT_ORDER_KEY, order);
    if (latest.status !== order.status) {
      order.status = latest.status;
      updateOrderTracking(latest);
    }
    return;
  }
  try {
    const response = await apiRequest(`/api/v1/orders/${encodeURIComponent(order.id)}`);
    if (!response.ok) return;
    const latest = { ...order, ...(await response.json()), source: 'checkout' };
    localStorage.setItem(CURRENT_ORDER_KEY, JSON.stringify(latest));
    updateOrderTracking(latest);
  } catch { /* Keep the last known status visible. */ }
}

async function renderOrders() {
  if (!getAccessToken()) {
    window.location.replace('/login');
    return;
  }
  window.clearInterval(orderRefreshTimer);
  const [profile, order] = await Promise.all([loadProfile(), loadTrackedOrder()]);
  if (!getAccessToken()) {
    window.location.replace('/login');
    return;
  }
  const firstName = escapeHtml(profile.name.split(' ')[0] || 'estudante');
  const initial = escapeHtml(profile.name.trim().charAt(0).toUpperCase() || 'L');
  const history = order.isDemo ? `<div><strong>#1039</strong><small>15 Jul 2026</small><span>X-Salada, Suco de laranja</span></div><div><strong>#1027</strong><small>18 Jul 2026</small><span>Coxinha, Água mineral</span></div><div><strong>#1012</strong><small>11 Jul 2026</small><span>Misto quente, Brownie</span></div>` : `<div><strong>${escapeHtml(orderIdLabel(order.id))}</strong><small>Pedido recente</small><span>${escapeHtml(orderItemsLabel(order))}</span></div>`;
  pageShell('orders', `<section class="page-heading"><h1>Olá, ${firstName} 👋</h1><p>Acompanhe seus pedidos e gerencie seus dados.</p></section><section class="orders-layout"><article class="profile-card"><div class="avatar">${initial}</div><div class="profile-name"><h2>${escapeHtml(profile.name)}</h2><span>Matrícula: ${escapeHtml(profile.studentId || 'Não informada')}</span></div><span class="student-status">Aluno cadastrado</span><p class="profile-label">Turma</p><strong>${escapeHtml(profile.className || 'Turma não informada')}</strong><button class="dark-action" type="button" data-edit-profile>Editar perfil</button></article><section class="current-order"><h2>Pedido atual</h2><div id="tracked-order-card">${trackingMarkup(order)}</div><p class="order-last-updated" id="order-last-updated">Acompanhe as atualizações da cantina aqui.</p><button class="refresh-order" type="button" data-refresh-order>Atualizar status</button></section><section class="order-history"><h2>Histórico de pedidos</h2><div class="history-list">${history}</div></section></section><section class="help-banner"><div><h2>Precisa de ajuda?</h2><p>Procure a equipe da cantina ou fale com a coordenação da escola.</p></div><button class="yellow-action" type="button" data-open-faq>Ver dúvidas frequentes</button></section>`);
  document.querySelector('[data-edit-profile]')?.addEventListener('click', () => openProfileModal(profile));
  document.querySelector('[data-refresh-order]')?.addEventListener('click', async (event) => {
    event.currentTarget.disabled = true;
    await refreshTrackedOrder(order);
    event.currentTarget.disabled = false;
  });
  if (order.source === 'checkout' && order.id) orderRefreshTimer = window.setInterval(() => refreshTrackedOrder(order), 10000);
}

function canteenOrderCard(order) {
  const studentName = order.student_name || order.student_email || order.student_id || 'Aluno';
  const itemCount = order.items?.reduce((total, item) => total + (item.quantity || 0), 0) || 0;
  return `<article class="canteen-order-card"><div class="canteen-order-heading"><div><span class="canteen-order-id">${escapeHtml(orderIdLabel(order.id))}</span><h2>${escapeHtml(studentName)}</h2><p>${itemCount} itens • ${money(order.total || 0)}</p></div><div class="canteen-pickup"><span>Retirada</span><strong>${escapeHtml(order.pickup_code || '—')}</strong></div></div><p class="canteen-order-items">${escapeHtml(orderItemsLabel(order))}</p><label class="canteen-status-label" for="status-${escapeHtml(order.id)}">Atualizar status<select id="status-${escapeHtml(order.id)}" data-order-id="${escapeHtml(order.id)}">${statusOptions(order.status)}</select></label></article>`;
}

function canteenMenuProduct(product) {
  const categories = { snacks: 'Lanche', drinks: 'Bebida', bakery: 'Padaria', sweets: 'Doce', healthy: 'Saudável' };
  return `<article class="canteen-menu-card"><div class="canteen-menu-preview">${productCard({ ...product, available: true }, true)}</div><form data-menu-form="${escapeHtml(product.id)}"><label>Nome<input name="name" value="${escapeHtml(product.name)}" required /></label><label>Descrição<input name="description" value="${escapeHtml(product.description || '')}" /></label><div class="canteen-form-row"><label>Preço (R$)<input name="price" type="number" min="0" step="0.01" value="${(product.price / 100).toFixed(2)}" required /></label><label>Categoria<select name="categoryId">${Object.entries(categories).map(([value, label]) => `<option value="${value}" ${value === product.categoryId ? 'selected' : ''}>${label}</option>`).join('')}</select></label></div><label class="canteen-checkbox"><input name="available" type="checkbox" ${product.available !== false ? 'checked' : ''} /> Disponível para os alunos</label><button class="primary-action" type="submit">Salvar item</button><p class="canteen-form-feedback" aria-live="polite"></p></form></article>`;
}

async function renderCanteenMenu() {
  pageShell('canteen-menu', `<section class="page-heading canteen-heading"><span class="section-kicker">CARDÁPIO</span><h1>Editar cardápio</h1><p>Atualize os itens que os alunos encontram no pedido.</p></section><section class="canteen-menu-grid" id="canteen-menu-grid"><p class="loading-note">Carregando itens...</p></section>`, { role: 'canteen' });
  const products = await loadProducts();
  const grid = document.querySelector('#canteen-menu-grid');
  grid.innerHTML = products.map(canteenMenuProduct).join('');
  grid.querySelectorAll('[data-asset]').forEach((image) => hydrateImages(image.parentElement));
  grid.querySelectorAll('[data-menu-form]').forEach((form) => form.addEventListener('submit', (event) => {
    event.preventDefault();
    const product = products.find((item) => item.id === form.dataset.menuForm);
    if (!product) return;
    const categoryLabels = { snacks: 'Lanche', drinks: 'Bebida', bakery: 'Padaria', sweets: 'Doce', healthy: 'Saudável' };
    const categoryId = form.elements.categoryId.value;
    saveCatalogProduct({ ...product, name: form.elements.name.value.trim(), description: form.elements.description.value.trim(), price: Math.round(Number(form.elements.price.value) * 100), categoryId, category: categoryLabels[categoryId], available: form.elements.available.checked });
    form.querySelector('.canteen-form-feedback').textContent = 'Item salvo para o cardápio dos alunos.';
  }));
}

function renderCanteenStudents() {
  const accounts = getLocalAccounts().filter((account) => account.role !== 'canteen');
  const students = accounts.length ? accounts.map((account) => `<article class="canteen-student-card"><div class="student-avatar">${escapeHtml((account.name || 'A').charAt(0).toUpperCase())}</div><div><h2>${escapeHtml(account.name || 'Aluno')}</h2><p>${escapeHtml(account.email || 'E-mail não informado')}</p><span>Matrícula: ${escapeHtml(account.studentId || 'Não informada')}</span><small>${escapeHtml(account.className || 'Turma não informada')}</small></div></article>`).join('') : '<div class="canteen-empty"><h2>Nenhum aluno cadastrado</h2><p>Os alunos criados neste navegador aparecerão aqui.</p></div>';
  pageShell('canteen-students', `<section class="page-heading canteen-heading"><span class="section-kicker">ALUNOS</span><h1>Alunos cadastrados</h1><p>Consulte os alunos que podem fazer pedidos nesta instalação.</p></section><div class="student-count"><strong>${accounts.length}</strong><span>alunos cadastrados</span></div><section class="canteen-student-grid">${students}</section>`, { role: 'canteen' });
}

function renderCanteen() {
  const activeUser = getActiveUser();
  if (!getAccessToken() || !isCanteenUser(activeUser)) {
    pageShell('canteen', `<section class="access-denied"><span class="section-kicker">ÁREA RESTRITA</span><h1>Acesso da cantina</h1><p>Entre com o usuário administrativo para confirmar pedidos e atualizar os status.</p><button class="primary-action" type="button" data-open-login>Entrar como cantina</button></section>`, { role: 'canteen' });
    return;
  }

  const view = new URLSearchParams(window.location.search).get('view') || 'orders';
  if (view === 'menu') { renderCanteenMenu(); return; }
  if (view === 'students') { renderCanteenStudents(); return; }

  pageShell('canteen', `<section class="page-heading canteen-heading"><span class="section-kicker">PAINEL DA CANTINA</span><h1>Pedidos para preparar</h1><p>Confirme cada pedido e atualize o status para o aluno acompanhar.</p></section><section class="canteen-stats"><article><strong id="canteen-total">0</strong><span>Pedidos recebidos</span></article><article><strong id="canteen-preparing">0</strong><span>Em preparo</span></article><article><strong id="canteen-ready">0</strong><span>Prontos para retirada</span></article></section><section class="canteen-orders" id="canteen-orders" aria-live="polite"></section>`, { role: 'canteen' });
  const render = () => {
    const orders = readLocalOrders();
    document.querySelector('#canteen-total').textContent = orders.length;
    document.querySelector('#canteen-preparing').textContent = orders.filter((order) => order.status === 'preparing').length;
    document.querySelector('#canteen-ready').textContent = orders.filter((order) => order.status === 'ready').length;
    document.querySelector('#canteen-orders').innerHTML = orders.length ? orders.map(canteenOrderCard).join('') : '<div class="canteen-empty"><h2>Nenhum pedido recebido</h2><p>Novos pedidos aparecerão aqui assim que um aluno confirmar a compra.</p></div>';
    document.querySelectorAll('[data-order-id]').forEach((select) => select.addEventListener('change', () => { updateLocalOrderStatus(select.dataset.orderId, select.value); render(); }));
  };
  render();
}

function initLanding() {
  document.body.classList.remove('app-screen');
  const navbarHost = document.querySelector('[data-shared-navbar]');
  if (navbarHost) navbarHost.outerHTML = `<div class="app-navbar-shell">${createNavbar('home', { showAuth: true, showLogout: Boolean(getAccessToken()) })}</div>`;
  if (window.location.hash === '#inicio') { window.history.replaceState(null, '', window.location.pathname + window.location.search); window.requestAnimationFrame(() => window.scrollTo(0, 0)); window.setTimeout(() => window.scrollTo(0, 0), 80); }
  const loadLocalImage = (selector, assetPath) => { const image = document.querySelector(selector); if (!image) return; fetch(assetPath).then((response) => response.text()).then((base64) => { image.src = `data:image/png;base64,${base64.trim()}`; }).catch(() => {}); };
  loadLocalImage('#hero-burger', '/assets/burger-figma.png.b64'); loadLocalImage('#buns-image', '/assets/buns.png.b64'); loadLocalImage('#cheese-image', '/assets/pao-de-queijo.png.b64'); loadLocalImage('#juice-image', '/assets/suco-laranja.png.b64');
}

initLoginModal({ onRegister: openRegisterModal });
initRegisterModal();
initProfileModal();
initFaqModal();
document.addEventListener('click', (event) => {
  const target = event.target instanceof Element ? event.target : null;
  if (!target?.closest('[data-logout]')) return;
  clearAccessToken();
  clearActiveUser();
  window.location.href = '/login';
});
document.addEventListener('profile-saved', () => { if (currentRoute() === '/meus-pedidos') renderOrders(); });
document.addEventListener('user-authenticated', () => { if (currentRoute() === '/meus-pedidos') renderOrders(); });
const route = currentRoute();
if (route === '/cardapio') renderMenu();
else if (route === '/carrinho') renderCart();
else if (route === '/meus-pedidos') renderOrders();
else if (route === '/cantina') renderCanteen();
else {
  initLanding();
  if (route === '/login') openLoginModal();
  if (route === '/cadastro') openRegisterModal();
}
