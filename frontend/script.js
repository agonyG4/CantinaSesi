import { createNavbar } from './navbar.js';
import { apiErrorMessage, apiRequest, getAccessToken } from './api.js';
import { initLoginModal, openLoginModal } from './login-modal.js';
import { initRegisterModal, openRegisterModal } from './register-modal.js';

const CART_KEY = 'sabor-digital-cart';
const PAYMENT_KEY = 'sabor-digital-payment';

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

function cartCount() { return readCart().reduce((total, item) => total + item.quantity, 0); }

function updateCartCount() {
  document.querySelectorAll('[data-cart-count]').forEach((node) => { node.textContent = cartCount() || ''; });
}

function addToCart(product) {
  if (!product) return;
  const cart = readCart();
  const existing = cart.find((item) => item.productId === product.id);
  if (existing) existing.quantity += 1;
  else cart.push({ productId: product.id, name: product.name, unitPrice: product.price, quantity: 1, description: product.description, image: product.image, tone: product.tone });
  saveCart(cart);
}

function currentRoute() { return window.location.pathname.replace(/\/$/, '') || '/'; }

function pageShell(active, content) {
  document.body.classList.add('app-screen');
  document.body.innerHTML = `<div class="app-screen-shell">${createNavbar(active)}<main class="app-main">${content}</main></div>`;
  document.title = `Sabor Digital | ${active === 'menu' ? 'Cardápio' : active === 'cart' ? 'Carrinho' : active === 'orders' ? 'Meus pedidos' : 'Cantina SESI'}`;
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
    return payload.items.map((item, index) => {
      const backendCategoryId = item.category_id || 'category_snacks';
      const categoryId = backendCategoryId.replace(/^category_/, '');
      return {
        ...item,
        category: item.category || categoryNames.get(backendCategoryId) || 'Lanche',
        categoryId,
        tone: fallbackProducts[index % fallbackProducts.length].tone,
        image: fallbackProducts[index % fallbackProducts.length].image,
      };
    });
  } catch {
    try {
      const response = await fetch('/data/catalog.json');
      if (!response.ok) throw new Error('local catalog unavailable');
      const payload = await response.json();
      if (!Array.isArray(payload.products) || !payload.products.length) throw new Error('empty local catalog');
      return payload.products;
    } catch { return fallbackProducts; }
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
    grid.querySelectorAll('[data-add-product]').forEach((button) => button.addEventListener('click', () => { addToCart(products.find((item) => item.id === button.dataset.addProduct)); button.textContent = 'Adicionado'; setTimeout(() => { button.textContent = 'Adicionar'; }, 900); }));
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
  document.querySelectorAll('#suggestion-row [data-add-product]').forEach((button) => button.addEventListener('click', () => { addToCart(products.find((item) => item.id === button.dataset.addProduct)); render(); }));
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
      const response = await apiRequest('/api/v1/orders', {
        method: 'POST',
        body: JSON.stringify({ items: cart.map((item) => ({ product_id: item.productId, quantity: item.quantity })) }),
      });
      if (!response.ok) throw new Error(await apiErrorMessage(response, 'Não foi possível enviar o pedido.'));
      const order = await response.json();
      localStorage.removeItem(CART_KEY);
      render();
      document.querySelector('#cart-feedback').textContent = `Pedido ${order.id} enviado para a cantina. Retire com o código ${order.pickup_code}. Pagamento: ${payment === 'pix' ? 'Pix' : 'cartão'}.`;
    } catch (error) {
      feedback.textContent = error.message || 'Não foi possível enviar o pedido.';
    } finally {
      button.disabled = false;
    }
  });
  render();
}

function renderOrders() {
  pageShell('orders', `<section class="page-heading"><h1>Olá, Lucas 👋</h1><p>Acompanhe seus pedidos e gerencie seus dados.</p></section><section class="orders-layout"><article class="profile-card"><div class="avatar">L</div><div class="profile-name"><h2>Lucas Almeida</h2><span>Matrícula: 20251234</span></div><span class="student-status">Aluno cadastrado</span><p class="profile-label">Turma</p><strong>3º ano • Desenvolvimento de Sistemas</strong><button class="dark-action">Editar perfil</button></article><section class="current-order"><h2>Pedido atual</h2><article class="current-order-card"><span class="order-status-pill">EM PREPARO</span><h3>Pedido #67</h3><p>3 itens • R$ 27,50</p><ol class="status-tracker"><li class="done">Pedido confirmado</li><li class="done">Em preparo</li><li>Pronto para retirada</li><li>Entregue</li></ol></article></section><section class="order-history"><h2>Histórico de pedidos</h2><div class="history-list"><div><strong>#1039</strong><small>15 Jul 2026</small><span>X-Salada, Suco de laranja</span></div><div><strong>#1027</strong><small>18 Jul 2026</small><span>Coxinha, Água mineral</span></div><div><strong>#1012</strong><small>11 Jul 2026</small><span>Misto quente, Brownie</span></div></div></section></section><section class="help-banner"><div><h2>Precisa de ajuda?</h2><p>Procure a equipe da cantina ou fale com a coordenação da escola.</p></div><button class="yellow-action">Ver dúvidas frequentes</button></section>`);
}

function initLanding() {
  document.body.classList.remove('app-screen');
  const navbarHost = document.querySelector('[data-shared-navbar]');
  if (navbarHost) navbarHost.outerHTML = `<div class="app-navbar-shell">${createNavbar('home', { showAuth: true })}</div>`;
  if (window.location.hash === '#inicio') { window.history.replaceState(null, '', window.location.pathname + window.location.search); window.requestAnimationFrame(() => window.scrollTo(0, 0)); window.setTimeout(() => window.scrollTo(0, 0), 80); }
  const loadLocalImage = (selector, assetPath) => { const image = document.querySelector(selector); if (!image) return; fetch(assetPath).then((response) => response.text()).then((base64) => { image.src = `data:image/png;base64,${base64.trim()}`; }).catch(() => {}); };
  loadLocalImage('#hero-burger', '/assets/burger-figma.png.b64'); loadLocalImage('#buns-image', '/assets/buns.png.b64'); loadLocalImage('#cheese-image', '/assets/pao-de-queijo.png.b64'); loadLocalImage('#juice-image', '/assets/suco-laranja.png.b64');
}

initLoginModal({ onRegister: openRegisterModal });
initRegisterModal();
const route = currentRoute();
if (route === '/cardapio') renderMenu();
else if (route === '/carrinho') renderCart();
else if (route === '/meus-pedidos') renderOrders();
else {
  initLanding();
  if (route === '/login') openLoginModal();
  if (route === '/cadastro') openRegisterModal();
}
