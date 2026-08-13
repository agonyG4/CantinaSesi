if (window.location.hash === '#inicio') {
  window.history.replaceState(null, '', window.location.pathname + window.location.search);
  window.requestAnimationFrame(() => window.scrollTo(0, 0));
  window.setTimeout(() => window.scrollTo(0, 0), 80);
}

document.querySelectorAll('.nav-link').forEach((link) => {
  link.addEventListener('click', () => {
    document.querySelectorAll('.nav-link').forEach((item) => {
      item.classList.remove('active');
      item.removeAttribute('aria-current');
    });
    link.classList.add('active');
    link.setAttribute('aria-current', 'page');
  });
});

const heroBurger = document.querySelector('#hero-burger');

fetch('/assets/burger-figma.png.b64')
  .then((response) => {
    if (!response.ok) throw new Error('Asset do Figma indisponível');
    return response.text();
  })
  .then((base64) => {
    heroBurger.src = `data:image/png;base64,${base64.trim()}`;
  })
  .catch(() => {
    // Mantém o SVG local como fallback quando o asset exportado não estiver disponível.
  });

const loadLocalImage = (selector, assetPath) => {
  const image = document.querySelector(selector);
  if (!image) return;
  fetch(assetPath)
    .then((response) => {
      if (!response.ok) throw new Error('Asset indisponível');
      return response.text();
    })
    .then((base64) => {
      image.src = `data:image/png;base64,${base64.trim()}`;
    })
    .catch(() => {});
};

loadLocalImage('#buns-image', '/assets/buns.png.b64');
loadLocalImage('#cheese-image', '/assets/pao-de-queijo.png.b64');
loadLocalImage('#juice-image', '/assets/suco-laranja.png.b64');

document.querySelectorAll('.add-button').forEach((button) => {
  button.addEventListener('click', () => {
    button.textContent = 'Adicionado';
    button.setAttribute('aria-label', 'Produto adicionado ao carrinho');
  });
});
