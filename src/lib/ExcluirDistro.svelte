<script lang="ts">
  import { api } from './api';
  import { estado } from './estado.svelte';
  import { bytes } from './formato';
  import type { Distro } from './tipos';

  // Excluir não tem volta: o nome digitado confirma, e a cópia opcional vem antes, de modo que
  // uma cópia que falhe deixa a distro como estava.
  let { distro = $bindable() }: { distro: Distro | null } = $props();

  let dialogo: HTMLDialogElement;
  let campoNome = $state<HTMLInputElement>();
  let digitado = $state('');
  let exportar = $state(false);
  let destino = $state('');
  let etapa = $state<string | null>(null);

  const confere = $derived(!!distro && digitado === distro.nome);
  const destinoOk = $derived(!exportar || /\.tar$/i.test(destino.trim()));

  function hoje(): string {
    const d = new Date();
    const dois = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${dois(d.getMonth() + 1)}-${dois(d.getDate())}`;
  }

  $effect(() => {
    if (distro && !dialogo.open) {
      const nome = distro.nome;
      digitado = '';
      exportar = false;
      destino = '';
      etapa = null;
      dialogo.showModal();
      campoNome?.focus();
      api.pastaDeDocumentos().then(
        (pasta) => {
          if (pasta && distro?.nome === nome && !destino) destino = `${pasta}\\${nome}-${hoje()}.tar`;
        },
        () => {},
      );
    } else if (!distro && dialogo.open) {
      dialogo.close();
    }
  });

  async function excluir() {
    if (!distro || !confere || !destinoOk) return;
    const nome = distro.nome;
    etapa = exportar ? 'Exportando a cópia; com a distro grande, isso leva minutos' : 'Excluindo';
    try {
      const r = await api.excluirDistro(nome, exportar ? destino.trim() : null);
      distro = null;
      estado.avisar(
        `${nome} foi excluída e liberou ${bytes(r.liberado_bytes)}.` + (r.exportado ? ` A cópia está em ${r.exportado}.` : ''),
      );
    } catch (e) {
      etapa = null;
      estado.avisar(`${nome}: ${e}`);
    } finally {
      estado.receberDistros(await api.listarDistros().catch(() => estado.distros));
    }
  }
</script>

<dialog
  bind:this={dialogo}
  onclose={() => (distro = null)}
  oncancel={(e) => {
    // Com a exclusão em andamento, fechar o diálogo não a interrompe; ele fica até o fim.
    if (etapa) e.preventDefault();
  }}
  aria-labelledby="excluir-titulo"
>
  {#if distro}
    <h2 id="excluir-titulo">Excluir {distro.nome}?</h2>
    <div class="corpo">
      <p>
        O disco de {distro.nome} é apagado com tudo o que há nele: arquivos, programas instalados e configurações.
        Não tem como desfazer.
        {#if distro.padrao}{distro.nome} é a distro padrão; o WSL passa a usar outra.{/if}
      </p>

      <label class="opcao">
        <input type="checkbox" bind:checked={exportar} disabled={!!etapa} />
        <span>Exportar uma cópia antes, num arquivo .tar</span>
      </label>
      {#if exportar}
        <label class="campo">
          <span>Onde salvar a cópia</span>
          <input type="text" bind:value={destino} disabled={!!etapa} aria-invalid={!destinoOk} spellcheck="false" />
        </label>
        <p class="nota">
          A cópia ocupa quase o mesmo que os arquivos da distro. Ela volta com
          <code>wsl --import</code>. Se a cópia falhar, nada é excluído.
        </p>
      {/if}

      <label class="campo">
        <span>Para confirmar, digite <strong>{distro.nome}</strong></span>
        <input type="text" bind:value={digitado} bind:this={campoNome} disabled={!!etapa} autocomplete="off" spellcheck="false" />
      </label>

      {#if etapa}
        <p class="nota" role="status">{etapa}.</p>
      {/if}
    </div>
    <div class="rodape">
      <button class="botao primario perigoso" disabled={!confere || !destinoOk || !!etapa} onclick={excluir}>Excluir</button>
      <button class="botao" disabled={!!etapa} onclick={() => (distro = null)}>Cancelar</button>
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(480px, calc(100vw - 32px));
    padding: 0;
    border: 1px solid var(--borda-forte);
    border-radius: 8px;
    background: var(--camada-forte);
    color: var(--texto);
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.28);
  }

  dialog::backdrop {
    background: rgba(0, 0, 0, 0.3);
  }

  h2 {
    font-size: 20px;
    font-weight: 600;
    margin: 24px 24px 12px;
  }

  .corpo {
    display: grid;
    gap: 14px;
    padding: 0 24px 20px;
    line-height: 1.45;
  }

  .corpo p {
    margin: 0;
  }

  .opcao {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .campo {
    display: grid;
    gap: 6px;
  }

  .campo span {
    color: var(--texto-2);
    font-size: 12px;
  }

  .campo strong {
    color: var(--texto);
  }

  .campo input {
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-bottom-color: var(--borda-forte);
    border-radius: var(--raio);
    background: var(--camada-forte);
  }

  .campo input[aria-invalid='true'] {
    border-bottom: 2px solid var(--perigo);
  }

  .nota {
    color: var(--texto-2);
    font-size: 12px;
  }

  .nota code {
    white-space: nowrap;
  }

  .rodape {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 20px 24px;
    background: var(--hover);
    border-top: 1px solid var(--borda);
  }

  .rodape .botao {
    justify-content: center;
  }

  .perigoso:not(:disabled) {
    background: var(--perigo);
    border-color: var(--perigo);
    color: var(--camada-forte);
  }
</style>
