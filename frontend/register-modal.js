const registerModalMarkup = `
  <div class="register-modal" data-register-modal hidden>
    <div class="register-backdrop" data-register-close></div>
    <section class="register-dialog" role="dialog" aria-modal="true" aria-labelledby="register-title" aria-describedby="register-description">
      <button class="register-close" type="button" data-register-close aria-label="Fechar cadastro">&times;</button>
      <div class="register-kicker">SABOR DIGITAL</div>
      <h2 id="register-title">Crie sua conta</h2>
      <p id="register-description">Cadastre-se para pedir antes do intervalo e acompanhar tudo em um só lugar.</p>
      <form class="register-form" data-register-form>
        <label for="register-name">Nome completo</label>
        <input id="register-name" name="name" type="text" autocomplete="name" placeholder="Digite seu nome completo" required />
        <div class="register-row">
          <div>
            <label for="register-email">E-mail</label>
            <input id="register-email" name="email" type="email" autocomplete="email" placeholder="voce@email.com" required />
          </div>
          <div>
            <label for="register-student-id">Matrícula</label>
            <input id="register-student-id" name="studentId" type="text" autocomplete="off" placeholder="Sua matrícula" required />
          </div>
        </div>
        <label for="register-password">Senha</label>
        <div class="register-password-field">
          <input id="register-password" name="password" type="password" autocomplete="new-password" placeholder="Mínimo de 8 caracteres" minlength="8" required />
          <button class="register-password-toggle" type="button" data-register-toggle-password aria-label="Mostrar senha" aria-pressed="false">Mostrar</button>
        </div>
        <label for="register-password-confirmation">Confirme sua senha</label>
        <input id="register-password-confirmation" name="passwordConfirmation" type="password" autocomplete="new-password" placeholder="Digite a senha novamente" minlength="8" required />
        <button class="register-submit" type="submit">Criar conta</button>
        <p class="register-status" data-register-status aria-live="polite"></p>
      </form>
      <div class="register-footer">
        <span>Já tem uma conta?</span>
        <button type="button" data-open-login>Entrar</button>
      </div>
    </section>
  </div>`;

function getModal() {
  let modal = document.querySelector('[data-register-modal]');
  if (!modal) {
    document.body.insertAdjacentHTML('beforeend', registerModalMarkup);
    modal = document.querySelector('[data-register-modal]');
  }
  return modal;
}

export function openRegisterModal() {
  const modal = getModal();
  modal.hidden = false;
  document.body.classList.add('modal-open');
  window.requestAnimationFrame(() => {
    modal.classList.add('is-open');
    modal.querySelector('#register-name')?.focus();
  });
}

function closeRegisterModal() {
  const modal = document.querySelector('[data-register-modal]');
  if (!modal) return;
  modal.classList.remove('is-open');
  document.body.classList.remove('modal-open');
  window.setTimeout(() => { modal.hidden = true; }, 180);
}

function togglePassword(toggle, input) {
  const isVisible = input.type === 'text';
  input.type = isVisible ? 'password' : 'text';
  toggle.textContent = isVisible ? 'Mostrar' : 'Ocultar';
  toggle.setAttribute('aria-label', isVisible ? 'Mostrar senha' : 'Ocultar senha');
  toggle.setAttribute('aria-pressed', String(!isVisible));
}

export function initRegisterModal() {
  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;

    const registerTrigger = target.closest('[data-open-register], a[href="/cadastro"]:not([data-login-register])');
    if (registerTrigger) {
      event.preventDefault();
      openRegisterModal();
      return;
    }

    if (target.closest('[data-register-close]')) {
      closeRegisterModal();
      return;
    }

    const toggle = target.closest('[data-register-toggle-password]');
    if (toggle) {
      const password = document.querySelector('#register-password');
      if (password) togglePassword(toggle, password);
      return;
    }

    if (target.closest('[data-open-login]') && document.querySelector('[data-register-modal].is-open')) {
      event.preventDefault();
      closeRegisterModal();
      window.setTimeout(() => document.querySelector('[data-open-login]')?.click(), 190);
    }
  });

  document.addEventListener('submit', (event) => {
    const form = event.target instanceof HTMLFormElement ? event.target : null;
    if (!form?.matches('[data-register-form]')) return;
    event.preventDefault();
    const password = form.querySelector('#register-password');
    const confirmation = form.querySelector('#register-password-confirmation');
    const status = form.querySelector('[data-register-status]');
    if (password.value !== confirmation.value) {
      confirmation.setCustomValidity('As senhas precisam ser iguais.');
      confirmation.reportValidity();
      confirmation.setCustomValidity('');
      if (status) status.textContent = 'Confira a confirmação da senha.';
      return;
    }
    if (status) status.textContent = 'A interface está pronta para conectar ao cadastro da escola.';
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && document.querySelector('[data-register-modal].is-open')) closeRegisterModal();
  });
}
