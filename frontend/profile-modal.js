import { getLocalAccount, saveLocalAccount } from './api.js';

const profileModalMarkup = `
  <div class="profile-modal" data-profile-modal hidden>
    <div class="profile-backdrop" data-profile-close></div>
    <section class="profile-dialog" role="dialog" aria-modal="true" aria-labelledby="profile-title" aria-describedby="profile-description">
      <button class="profile-close" type="button" data-profile-close aria-label="Fechar edição de perfil">&times;</button>
      <div class="profile-kicker">SEU PERFIL</div>
      <h2 id="profile-title">Editar perfil</h2>
      <p id="profile-description">Mantenha seus dados atualizados para facilitar a retirada dos pedidos.</p>
      <form class="profile-form" data-profile-form>
        <label for="profile-name">Nome completo</label>
        <input id="profile-name" name="name" type="text" autocomplete="name" required />
        <label for="profile-email">E-mail</label>
        <input id="profile-email" name="email" type="email" autocomplete="email" readonly />
        <div class="profile-form-row">
          <div>
            <label for="profile-student-id">Matrícula</label>
            <input id="profile-student-id" name="studentId" type="text" autocomplete="off" required />
          </div>
          <div>
            <label for="profile-class">Turma</label>
            <input id="profile-class" name="className" type="text" required />
          </div>
        </div>
        <button class="profile-submit" type="submit">Salvar alterações</button>
        <p class="profile-status" data-profile-status aria-live="polite"></p>
      </form>
    </section>
  </div>`;

function getModal() {
  let modal = document.querySelector('[data-profile-modal]');
  if (!modal) {
    document.body.insertAdjacentHTML('beforeend', profileModalMarkup);
    modal = document.querySelector('[data-profile-modal]');
  }
  return modal;
}

export function openProfileModal(profile = {}) {
  const modal = getModal();
  const form = modal.querySelector('[data-profile-form]');
  form.dataset.profileKey = profile.profileKey || `sabor-digital-profile:${profile.email || 'default'}`;
  form.elements.name.value = profile.name || '';
  form.elements.email.value = profile.email || '';
  form.elements.studentId.value = profile.studentId || '';
  form.elements.className.value = profile.className || '';
  modal.hidden = false;
  document.body.classList.add('modal-open');
  window.requestAnimationFrame(() => {
    modal.classList.add('is-open');
    form.elements.name.focus();
  });
}

function closeProfileModal() {
  const modal = document.querySelector('[data-profile-modal]');
  if (!modal) return;
  modal.classList.remove('is-open');
  document.body.classList.remove('modal-open');
  window.setTimeout(() => { modal.hidden = true; }, 180);
}

export function initProfileModal() {
  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('[data-profile-close]')) closeProfileModal();
  });

  document.addEventListener('submit', (event) => {
    const form = event.target instanceof HTMLFormElement ? event.target : null;
    if (!form?.matches('[data-profile-form]')) return;
    event.preventDefault();
    const profile = {
      name: form.elements.name.value.trim(),
      email: form.elements.email.value.trim(),
      studentId: form.elements.studentId.value.trim(),
      className: form.elements.className.value.trim(),
    };
    localStorage.setItem(form.dataset.profileKey || `sabor-digital-profile:${profile.email}`, JSON.stringify(profile));
    const account = getLocalAccount(profile.email);
    if (account) saveLocalAccount({ ...account, name: profile.name, studentId: profile.studentId, className: profile.className });
    form.querySelector('[data-profile-status]').textContent = 'Perfil atualizado.';
    closeProfileModal();
    document.dispatchEvent(new CustomEvent('profile-saved', { detail: profile }));
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && document.querySelector('[data-profile-modal].is-open')) closeProfileModal();
  });
}
