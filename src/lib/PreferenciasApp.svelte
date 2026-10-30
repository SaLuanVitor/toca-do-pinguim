<script lang="ts">
  // Preferências do próprio app: a busca de atualizações e a tecla de atalho do Windows que
  // abre a Toca do Pinguim, gravada no atalho do menu Iniciar.
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
  let procurando = $state(false);
  let resultadoBusca = $state('');

  async function mudarDevolverSozinho(caixa: HTMLInputElement) {
    const sim = caixa.checked;
    try {
      await api.definirDevolverCacheAutomatico(sim);
      estado.devolverCacheAutomatico = sim;
    } catch (e) {
      // A preferência não mudou; a caixa volta a mostrar o que vale.
      caixa.checked = !sim;
      estado.avisar(String(e));
    }
  }

  async function procurarAgora() {
    procurando = true;
    resultadoBusca = '';
    try {
      const nova = await api.procurarAtualizacao();
      resultadoBusca = nova ? `Versão ${nova.versao} disponível.` : 'Você já está na versão mais recente.';
    } catch (e) {
      resultadoBusca = `Não foi possível procurar: ${e}`;
    } finally {
      procurando = false;
    }
  }

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

  <section aria-labelledby="privacidade-titulo">
    <h2 id="privacidade-titulo">Atualizações e privacidade</h2>
    <label class="opcao">
      <input
        type="checkbox"
        checked={estado.procurarAtualizacoes}
        onchange={(e) => estado.definirProcurarAtualizacoes(e.currentTarget.checked)}
      />
      Procurar versão nova ao abrir o app, a cada hora e ao voltar para ele
    </label>
    <p class="explica">
      A busca é o único acesso do app à internet: ele consulta as versões em github.com, e o GitHub
      recebe o endereço IP deste computador, como em qualquer visita a um site. Nada mais sai daqui.
      O log, o histórico de memória e a atividade ficam só nesta máquina.
    </p>
    <div class="acoes">
      <button class="botao" onclick={procurarAgora} disabled={procurando}>{procurando ? 'Procurando' : 'Procurar agora'}</button>
      {#if resultadoBusca}<span role="status">{resultadoBusca}</span>{/if}
    </div>
  </section>

  <section aria-labelledby="cache-titulo">
    <h2 id="cache-titulo">Memória</h2>
    <label class="opcao">
      <input
        type="checkbox"
        checked={estado.devolverCacheAutomatico}
        onchange={(e) => mudarDevolverSozinho(e.currentTarget)}
      />
      Devolver o cache da VM sozinho quando o Windows estiver sem memória
    </label>
    <p class="explica">
      Abaixo de 10% de memória livre no Windows, o app esvazia o cache do Linux, no máximo uma vez a cada 10 minutos, e
      registra na Atividade quanto voltou. Medido numa máquina com Docker e CI rodando, voltaram 2,9 GB na hora e 1,3 GB
      continuavam livres 5 minutos depois. Nada se perde: a VM só relê do disco o que precisar. Para a VM ocupar menos de
      vez, o caminho é baixar o limite de memória dela, em Desempenho.
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

  .opcao {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    font-size: 13px;
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
