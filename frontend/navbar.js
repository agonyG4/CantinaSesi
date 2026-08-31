const navigationLinks = [
  ['Início', '/', 'home'],
  ['Cardápio', '/cardapio', 'menu'],
  ['Meus pedidos', '/meus-pedidos', 'orders'],
];

function getCartCount() {
  try {
    const cart = JSON.parse(localStorage.getItem('sabor-digital-cart') || '[]');
    return cart.reduce((total, item) => total + item.quantity, 0);
  } catch {
    return 0;
  }
}

export function createNavbar(activePage = '', { showAuth = false } = {}) {
  const links = navigationLinks
    .map(([label, href, key]) => `<a class="app-nav-link ${activePage === key ? 'is-active' : ''}" href="${href}">${label}</a>`)
    .join('');

  const cartCount = getCartCount();
  const authActions = showAuth ? '<div class="app-auth-actions"><button class="app-auth-link" type="button" data-open-login>Entrar</button><a class="app-auth-signup" href="/cadastro">Cadastrar-se</a></div>' : '';
  return `<header class="app-header"><a class="app-brand" href="/">Sabor Digital</a><nav class="app-nav" aria-label="Navegação principal">${links}</nav>${authActions}<a class="app-cart-link ${activePage === 'cart' ? 'is-active' : ''}" href="/carrinho">Carrinho<span data-cart-count>${cartCount || ''}</span></a></header>`;
}
