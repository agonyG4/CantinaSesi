const navigationLinks = [
  ['Início', '/', 'home'],
  ['Cardápio', '/cardapio', 'menu'],
  ['Meus pedidos', '/meus-pedidos', 'orders'],
];

const canteenNavigationLinks = [
  ['Pedidos', '/cantina', 'canteen'],
  ['Editar cardápio', '/cantina?view=menu', 'canteen-menu'],
  ['Alunos cadastrados', '/cantina?view=students', 'canteen-students'],
];

function getCartCount() {
  try {
    const cart = JSON.parse(localStorage.getItem('sabor-digital-cart') || '[]');
    return cart.reduce((total, item) => total + item.quantity, 0);
  } catch {
    return 0;
  }
}

export function createNavbar(activePage = '', { showAuth = false, role = 'student', showLogout = false } = {}) {
  const links = (role === 'canteen' ? canteenNavigationLinks : navigationLinks)
    .map(([label, href, key]) => `<a class="app-nav-link ${activePage === key ? 'is-active' : ''}" href="${href}">${label}</a>`)
    .join('');

  const cartCount = getCartCount();
  const authActions = showAuth && !showLogout ? '<div class="app-auth-actions"><button class="app-auth-link" type="button" data-open-login>Entrar</button><a class="app-auth-signup" href="/cadastro">Cadastrar-se</a></div>' : '';
  const cartLink = role === 'canteen' ? '' : `<a class="app-cart-link ${activePage === 'cart' ? 'is-active' : ''}" href="/carrinho">Carrinho <span data-cart-count>${cartCount || ''}</span></a>`;
  const logoutAction = showLogout ? '<button class="app-logout" type="button" data-logout>Sair</button>' : '';
  return `<header class="app-header"><a class="app-brand" href="${role === 'canteen' ? '/cantina' : '/'}">${role === 'canteen' ? 'Sabor Digital · Cantina' : 'Sabor Digital'}</a><nav class="app-nav" aria-label="Navegação principal">${links}</nav>${authActions}${cartLink}${logoutAction}</header>`;
}
