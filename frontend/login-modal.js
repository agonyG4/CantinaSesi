import { apiErrorMessage, apiRequest, authenticateLocalUser, isCanteenUser, saveActiveUser, setAccessToken } from './api.js';

const loginModalMarkup = `
  <div class="login-modal" data-login-modal hidden>
    <div class="login-backdrop" data-login-close></div>
    <section class="login-dialog" role="dialog" aria-modal="true" aria-labelledby="login-title" aria-describedby="login-description">
      <button class="login-close" type="button" data-login-close aria-label="Fechar login">&times;</button>
      <div class="login-kicker">SABOR DIGITAL</div>
      <h2 id="login-title">Entre na sua conta</h2>
      <p id="login-description">Acompanhe seus pedidos e monte seu lanche com antecedência.</p>
      <form class="login-form" data-login-form>
        <label for="login-email">E-mail ou matrícula</label>
        <input id="login-email" name="email" type="text" autocomplete="username" placeholder="Digite seu e-mail ou matrícula" required />
        <label for="login-password">Senha</label>
        <div class="login-password-field">
          <input id="login-password" name="password" type="password" autocomplete="current-password" placeholder="Digite sua senha" required />
          <button class="login-password-toggle" type="button" data-login-toggle-password aria-label="Mostrar senha" aria-pressed="false">Mostrar</button>
        </div>
        <button class="login-submit" type="submit">Entrar</button>
        <p class="login-status" data-login-status aria-live="polite"></p>
      </form>
      <div class="login-footer">
        <span>Primeiro acesso?</span>
        <a href="/cadastro" data-login-register>Crie sua conta</a>
      </div>
    </section>
  </div>`;

function getModal() {
  let modal = document.querySelector('[data-login-modal]');
  if (!modal) {
    document.body.insertAdjacentHTML('beforeend', loginModalMarkup);
    modal = document.querySelector('[data-login-modal]');
  }
  return modal;
}

export function openLoginModal() {
  const modal = getModal();
  modal.hidden = false;
  document.body.classList.add('modal-open');
  window.requestAnimationFrame(() => {
    modal.classList.add('is-open');
    modal.querySelector('#login-email')?.focus();
  });
}

function closeLoginModal() {
  const modal = document.querySelector('[data-login-modal]');
  if (!modal) return;
  modal.classList.remove('is-open');
  document.body.classList.remove('modal-open');
  window.setTimeout(() => { modal.hidden = true; }, 180);
}

export function initLoginModal({ onRegister } = {}) {
  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;

    const loginTrigger = target.closest('[data-open-login], a[href="/login"]');
    if (loginTrigger && !target.closest('[data-register-modal]')) {
      event.preventDefault();
      openLoginModal();
      return;
    }

    if (target.closest('[data-login-close]')) {
      closeLoginModal();
      return;
    }

    const toggle = target.closest('[data-login-toggle-password]');
    if (toggle) {
      const password = document.querySelector('#login-password');
      const isVisible = password.type === 'text';
      password.type = isVisible ? 'password' : 'text';
      toggle.textContent = isVisible ? 'Mostrar' : 'Ocultar';
      toggle.setAttribute('aria-label', isVisible ? 'Mostrar senha' : 'Ocultar senha');
      toggle.setAttribute('aria-pressed', String(!isVisible));
      return;
    }

    if (target.closest('[data-login-register]')) {
      event.preventDefault();
      closeLoginModal();
      window.setTimeout(() => onRegister?.(), 190);
    }
  });

  document.addEventListener('submit', (event) => {
    const form = event.target instanceof HTMLFormElement ? event.target : null;
    if (!form?.matches('[data-login-form]')) return;
    event.preventDefault();
    const status = form.querySelector('[data-login-status]');
    const submit = form.querySelector('.login-submit');
    if (submit) submit.disabled = true;
    if (status) status.textContent = 'Entrando...';

    (async () => {
      let apiFailure = new Error('E-mail ou senha inválidos.');
      try {
        const response = await apiRequest('/api/v1/auth/login', {
          method: 'POST',
          body: JSON.stringify({
            email: form.elements.email.value.trim(),
            password: form.elements.password.value,
          }),
        });
        if (response.ok) {
          const payload = await response.json();
          setAccessToken(payload.access_token);
          saveActiveUser(payload.user);
          document.dispatchEvent(new CustomEvent('user-authenticated', { detail: payload.user }));
          if (status) status.textContent = `Bem-vindo, ${payload.user?.name || 'estudante'}!`;
          window.setTimeout(() => { closeLoginModal(); if (isCanteenUser(payload.user)) window.location.href = '/cantina'; }, 600);
          return;
        }
        apiFailure = new Error(await apiErrorMessage(response, 'E-mail ou senha inválidos.'));
      } catch (error) {
        apiFailure = error;
      }

      const localUser = await authenticateLocalUser(form.elements.email.value, form.elements.password.value);
      if (!localUser) throw apiFailure;
      setAccessToken(`local:${localUser.id}`);
      saveActiveUser(localUser);
      document.dispatchEvent(new CustomEvent('user-authenticated', { detail: localUser }));
      if (status) status.textContent = `Bem-vindo, ${localUser.name}!`;
      window.setTimeout(() => { closeLoginModal(); if (isCanteenUser(localUser)) window.location.href = '/cantina'; }, 600);
    })()
      .catch((error) => { if (status) status.textContent = error.message || 'Não foi possível entrar agora.'; })
      .finally(() => {
        if (submit) submit.disabled = false;
      });
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && document.querySelector('[data-login-modal].is-open')) closeLoginModal();
  });
}
