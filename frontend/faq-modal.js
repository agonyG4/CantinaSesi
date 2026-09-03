const faqModalMarkup = `
  <div class="faq-modal" data-faq-modal hidden>
    <div class="faq-backdrop" data-faq-close></div>
    <section class="faq-dialog" role="dialog" aria-modal="true" aria-labelledby="faq-title">
      <button class="faq-close" type="button" data-faq-close aria-label="Fechar dúvidas frequentes">&times;</button>
      <div class="faq-kicker">AJUDA RÁPIDA</div>
      <h2 id="faq-title">Dúvidas frequentes</h2>
      <p class="faq-intro">Tudo o que você precisa saber para pedir no intervalo.</p>
      <details open><summary>Como acompanho meu pedido?</summary><p>Acesse “Meus pedidos”. O status é atualizado automaticamente e você também pode tocar em “Atualizar status”.</p></details>
      <details><summary>Quando posso retirar o lanche?</summary><p>Espere o status mudar para “Pronto para retirada” e mostre o número do pedido no balcão da cantina.</p></details>
      <details><summary>O que faço se não conseguir entrar?</summary><p>Confira o e-mail e a senha. Se você criou uma conta neste MVP, use os mesmos dados cadastrados neste navegador.</p></details>
      <details><summary>Como altero meus dados?</summary><p>Em “Meus pedidos”, toque em “Editar perfil” para atualizar nome, matrícula e turma.</p></details>
    </section>
  </div>`;

function getModal() {
  let modal = document.querySelector('[data-faq-modal]');
  if (!modal) {
    document.body.insertAdjacentHTML('beforeend', faqModalMarkup);
    modal = document.querySelector('[data-faq-modal]');
  }
  return modal;
}

function closeFaqModal() {
  const modal = document.querySelector('[data-faq-modal]');
  if (!modal) return;
  modal.classList.remove('is-open');
  document.body.classList.remove('modal-open');
  window.setTimeout(() => { modal.hidden = true; }, 180);
}

export function initFaqModal() {
  document.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('[data-open-faq]')) {
      const modal = getModal();
      modal.hidden = false;
      document.body.classList.add('modal-open');
      window.requestAnimationFrame(() => modal.classList.add('is-open'));
    }
    if (target?.closest('[data-faq-close]')) closeFaqModal();
  });
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape' && document.querySelector('[data-faq-modal].is-open')) closeFaqModal();
  });
}
