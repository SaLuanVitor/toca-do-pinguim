<script lang="ts">
  // Preferências do próprio app. Por ora, a tecla de atalho do Windows que abre a Toca do
  // Pinguim, gravada no atalho do menu Iniciar.
  import { onMount } from 'svelte';
  import { api } from './api';
  import { estado } from './estado.svelte';

  // O Windows aceita a tecla de um atalho com pelo menos dois modificadores.
  const MODIFICADORES = ['Ctrl', 'Alt', 'Shift'] as const;

  let versao = $state('');
  let atual = $state('');
  let caminho = $state('');
  let existe = $state(false);
  let nova = $state('');
  let aviso = $state('');
  let salvando = $state(false);
  let gravado = $state('');

  /** "Alt+Ctrl+P" do Windows vira "Ctrl+Alt+P": a ordem de quem lê. */
  function normalizar(tecla: string): string {
    const partes = tecla.split('+').filter(Boolean);
    const mods = MODIFICADORES.filter((m) => partes.some((p) => p.toLowerCase() === m.toLowerCase()));
    const resto = partes.filter((p) => !MODIFICADORES.some((m) => m.toLowerCase() === p.toLowerCase()));
    return [...mods, ...resto].join('+');
  }

  function teclaDe(e: KeyboardEvent): string | null {
    if (/^Key[A-Z]$/.test(e.code)) return e.code.slice(3);
    if (/^Digit\d$/.test(e.code)) return e.code.slice(5);
    if (/^F([1-9]|1[0-2])$/.test(e.code)) return e.code;
    return null;
  }

  function capturar(e: KeyboardEvent) {
    const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean) as string[];
    // Tab sem modificador continua navegando; Esc desiste da captura.
    if (e.key === 'Tab' && mods.length === 0) return;
    e.preventDefault();
    if (e.key === 'Escape') {
      nova = '';
      aviso = '';
      return;
    }
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return;
    const tecla = teclaDe(e);
    if (!tecla) {
      aviso = 'Use uma letra, um número ou uma tecla F1 a F12.';
      return;
    }
    if (mods.length < 2) {
      aviso = `Combine ${tecla} com dois modificadores, como Ctrl+Alt+${tecla}.`;
      return;
    }
    nova = [...mods, tecla].join('+');
    aviso = '';
  }

  async function carregar() {
    try {
      const r = await api.lerAtalhoTeclado();
      atual = normalizar(r.tecla);
      caminho = r.caminho;
      existe = r.existe;
    } catch (e) {
      estado.avisar(`Não foi possível ler a tecla de atalho: ${e}`);
    }
  }

  async function gravar(tecla: string) {
    salvando = true;
    gravado = '';
    try {
      await api.definirAtalhoTeclado(tecla);
      await carregar();
      nova = '';
      gravado = tecla ? `Pronto: ${normalizar(tecla)} abre a Toca do Pinguim.` : 'Tecla removida.';
    } catch (e) {
      estado.avisar(String(e));
    } finally {
      salvando = false;
    }
  }

  onMount(() => {
    carregar();
    api.versaoApp().then((v) => (versao = v), () => (versao = 'desconhecida'));
  });
</script>

<div class="rolagem">
  <section aria-labelledby="sobre-titulo">
    <h2 id="sobre-titulo">Sobre</h2>
    <div class="linha">
      <span class="rotulo">Versão</span>
      <strong>{versao}</strong>
    </div>
    <p class="explica">
      Código, versões e relato de bugs em
      <code>github.com/SaLuanVitor/toca-do-pinguim</code>. Licença MIT; as licenças das bibliotecas
      de terceiros incluídas estão no <code>THIRD-PARTY-NOTICES.md</code> de cada versão.
    </p>
  </section>

  <section aria-labelledby="tecla-titulo">
    <h2 id="tecla-titulo">Tecla de atalho</h2>
    <p class="explica">
      Abre o app de qualquer lugar do Windows, mesmo fechado. Com ele já aberto, traz a janela
      para frente. Quem escuta a tecla é o próprio Windows: nada fica rodando em segundo plano.
    </p>

    <div class="linha">
      <span class="rotulo">Tecla atual</span>
      <strong>{atual || 'nenhuma'}</strong>
    </div>

    <div class="linha">
      <label class="rotulo" for="captura">Nova tecla</label>
      <div class="captura">
        <input
          id="captura"
          readonly
          value={nova}
          placeholder="Clique aqui e pressione a combinação"
          onkeydown={capturar}
          aria-describedby="captura-ajuda{aviso ? ' captura-aviso' : ''}"
        />
        <span id="captura-ajuda" class="ajuda">
          Ctrl, Alt ou Shift, dois deles, mais uma letra, um número ou F1 a F12. Esc limpa.
        </span>
        {#if aviso}<span id="captura-aviso" class="aviso" role="alert">{aviso}</span>{/if}
      </div>
    </div>

    <div class="acoes">
      <button class="botao primario" onclick={() => gravar(nova)} disabled={!nova || salvando}>Gravar tecla</button>
      <button class="botao" onclick={() => gravar('')} disabled={!atual || salvando}>Remover tecla</button>
      {#if gravado}<span role="status">{gravado}</span>{/if}
    </div>

    <p class="explica">
      A tecla fica no atalho <code>{caminho}</code>{existe ? '' : ', que é criado ao gravar e aponta para este executável'}.
      Se outro programa já usa a mesma combinação, o Windows dá preferência a quem a registrou primeiro.
    </p>
  </section>
</div>

<style>
  .rolagem {
    flex: 1;
    overflow: auto;
    padding: 0 20px 20px;
  }

  h2 {
    margin: 12px 0 4px;
    font-size: 14px;
    font-weight: 600;
  }

  .explica {
    max-width: 640px;
    color: var(--texto-2);
    font-size: 13px;
  }

  .linha {
    display: grid;
    grid-template-columns: 120px minmax(0, 360px);
    gap: 12px;
    align-items: start;
    padding: 8px 0;
  }

  .rotulo {
    color: var(--texto-2);
    font-size: 13px;
    padding-top: 6px;
  }

  .captura {
    display: grid;
    gap: 4px;
  }

  .captura input {
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--borda);
    border-radius: var(--raio);
    background: var(--camada-forte);
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  .ajuda {
    color: var(--texto-2);
    font-size: 12px;
  }

  .aviso {
    color: var(--perigo);
    font-size: 12px;
  }

  .acoes {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
    font-size: 13px;
  }

  @media (max-width: 560px) {
    .linha {
      grid-template-columns: 1fr;
      gap: 4px;
    }
  }
</style>
