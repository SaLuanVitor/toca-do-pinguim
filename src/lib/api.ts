import type {
  Amostra,
  AnalisePastas,
  Discos,
  EspacoDocker,
  Compactacao,
  SegundoPlano,
  AtalhoTeclado,
  Atualizacao,
  ConteudoWslconfig,
  Devolucao,
  Distro,
  Docker,
  EstadoRecursos,
  LimpezaDocker,
  LimpezaProgramada,
  Limites,
  Evento,
  MemoriaWindows,
  MudancaWslconfig,
  ResumoMemoria,
  SituacaoWsl,
  Suspeito,
} from './tipos';
import { estado } from './estado.svelte';
import { criarDemonstracao } from './demonstracao';

export interface Api {
  listarDistros(): Promise<Distro[]>;
  iniciarDistro(nome: string): Promise<void>;
  terminarDistro(nome: string): Promise<void>;
  reiniciarDistro(nome: string): Promise<void>;
  desligarWsl(): Promise<void>;
  enviarSinal(distro: string, pid: number, sinal: number, nome: string): Promise<void>;
  definirIntervalo(ms: number): Promise<void>;
  listarAtividade(): Promise<Evento[]>;
  resumoMemoria(): Promise<ResumoMemoria>;
  definirLimiteMemoria(gb: number, reiniciar: boolean): Promise<void>;
  estadoRecursos(): Promise<EstadoRecursos>;
  pararConteineres(ids: string[], rotulo: string): Promise<void>;
  iniciarConteineres(ids: string[], rotulo: string): Promise<void>;
  reiniciarConteineres(ids: string[], rotulo: string): Promise<void>;
  devolverCache(): Promise<void>;
  definirDevolucao(automatico: boolean, minimoLivrePct: number, cacheMaximoGb: number | null): Promise<void>;
  lerDiscos(): Promise<Discos>;
  lerEspacoDocker(forcar: boolean): Promise<EspacoDocker>;
  analisarPastas(distro: string, ligar: boolean): Promise<AnalisePastas>;
  apagarPastas(distro: string, caminhos: string[]): Promise<number>;
  limparDocker(qual: LimpezaDocker, volumes: string[]): Promise<number>;
  limparParadosDoProjeto(projeto: string): Promise<number>;
  compactarDisco(caminho: string): Promise<Compactacao>;
  definirLimpeza(limpeza: LimpezaProgramada): Promise<void>;
  procurarAtualizacao(): Promise<Atualizacao | null>;
  instalarAtualizacao(): Promise<void>;
  abrirPaginaDaVersao(versao: string): Promise<void>;
  lerLog(linhas: number): Promise<string[]>;
  lerWslconfig(): Promise<ConteudoWslconfig>;
  gravarWslconfig(mudancas: MudancaWslconfig[], reiniciar: boolean): Promise<void>;
  lerWslconf(distro: string): Promise<ConteudoWslconfig>;
  versaoApp(): Promise<string>;
  lerAtalhoTeclado(): Promise<AtalhoTeclado>;
  definirAtalhoTeclado(tecla: string): Promise<void>;
  lerSegundoPlano(): Promise<SegundoPlano>;
  definirSegundoPlano(fecharParaBandeja: boolean, iniciarComWindows: boolean): Promise<void>;
  gravarWslconf(distro: string, mudancas: MudancaWslconfig[], reiniciar: boolean): Promise<void>;
  abrirPastaDeLogs(): Promise<void>;
  limitesDaMaquina(): Promise<Limites>;
}

export const dentroDoApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export let api: Api;

async function conectarTauri(): Promise<Api> {
  const { invoke } = await import('@tauri-apps/api/core');
  const { listen } = await import('@tauri-apps/api/event');

  await listen<Distro[]>('distros', (e) => estado.receberDistros(e.payload));
  await listen<{ distro: string; amostra: Amostra }>('amostra', (e) =>
    estado.receberAmostra(e.payload.distro, e.payload.amostra),
  );
  await listen<{ distro: string; ok: boolean; erro: string | null }>('resposta', (e) => {
    if (!e.payload.ok) estado.avisar(`${e.payload.distro}: ${e.payload.erro}`);
  });
  await listen<string>('agente_parou', (e) => estado.agenteParou(e.payload));
  await listen<string>('distro_ociosa', (e) => estado.distroOciosa(e.payload));
  await listen<Evento>('atividade', (e) => estado.receberEvento(e.payload));
  await listen<MemoriaWindows>('memoria_windows', (e) => (estado.memoriaWindows = e.payload));
  await listen<Docker>('docker', (e) => (estado.docker = e.payload));
  await listen<SituacaoWsl | null>('saude_wsl', (e) => (estado.saude = e.payload));
  await listen<Devolucao>('devolucao', (e) => (estado.devolucao = e.payload));
  await listen<{ distro: string; lista: Suspeito[] }>('suspeitos', (e) => estado.receberSuspeitos(e.payload.distro, e.payload.lista));
  await listen<{ distro: string | null; mensagem: string }>('erro', (e) =>
    estado.avisar(e.payload.distro ? `${e.payload.distro}: ${e.payload.mensagem}` : e.payload.mensagem),
  );

  return {
    listarDistros: () => invoke('listar_distros'),
    iniciarDistro: (nome) => invoke('iniciar_distro', { nome }),
    terminarDistro: (nome) => invoke('terminar_distro', { nome }),
    reiniciarDistro: (nome) => invoke('reiniciar_distro', { nome }),
    desligarWsl: () => invoke('desligar_wsl'),
    enviarSinal: (distro, pid, sinal, nome) => invoke('enviar_sinal', { distro, pid, sinal, nome }),
    definirIntervalo: (ms) => invoke('definir_intervalo', { ms }),
    listarAtividade: () => invoke('listar_atividade'),
    resumoMemoria: () => invoke('resumo_memoria'),
    definirLimiteMemoria: (gb, reiniciar) => invoke('definir_limite_memoria', { gb, reiniciar }),
    estadoRecursos: () => invoke('estado_recursos'),
    pararConteineres: (ids, rotulo) => invoke('parar_conteineres', { ids, rotulo }),
    iniciarConteineres: (ids, rotulo) => invoke('iniciar_conteineres', { ids, rotulo }),
    reiniciarConteineres: (ids, rotulo) => invoke('reiniciar_conteineres', { ids, rotulo }),
    devolverCache: () => invoke('devolver_cache'),
    definirDevolucao: (automatico, minimoLivrePct, cacheMaximoGb) =>
      invoke('definir_devolucao', { automatico, minimoLivrePct, cacheMaximoGb }),
    lerDiscos: () => invoke('ler_discos'),
    lerEspacoDocker: (forcar) => invoke('ler_espaco_docker', { forcar }),
    analisarPastas: (distro, ligar) => invoke('analisar_pastas', { distro, ligar }),
    apagarPastas: (distro, caminhos) => invoke('apagar_pastas', { distro, caminhos }),
    limparDocker: (qual, volumes) => invoke('limpar_docker', { qual, volumes }),
    limparParadosDoProjeto: (projeto) => invoke('limpar_parados_do_projeto', { projeto }),
    compactarDisco: (caminho) => invoke('compactar_disco', { caminho }),
    definirLimpeza: (limpeza) => invoke('definir_limpeza', { limpeza }),
    procurarAtualizacao: () => invoke('procurar_atualizacao'),
    instalarAtualizacao: () => invoke('instalar_atualizacao'),
    abrirPaginaDaVersao: (versao) => invoke('abrir_pagina_da_versao', { versao }),
    lerLog: (linhas) => invoke('ler_log', { linhas }),
    lerWslconfig: () => invoke('ler_wslconfig'),
    gravarWslconfig: (mudancas, reiniciar) => invoke('gravar_wslconfig', { mudancas, reiniciar }),
    lerWslconf: (distro) => invoke('ler_wslconf', { distro }),
    versaoApp: async () => (await import('@tauri-apps/api/app')).getVersion(),
    lerAtalhoTeclado: () => invoke('ler_atalho_teclado'),
    definirAtalhoTeclado: (tecla) => invoke('definir_atalho_teclado', { tecla }),
    lerSegundoPlano: () => invoke('ler_segundo_plano'),
    definirSegundoPlano: (fecharParaBandeja, iniciarComWindows) =>
      invoke('definir_segundo_plano', { fecharParaBandeja, iniciarComWindows }),
    gravarWslconf: (distro, mudancas, reiniciar) => invoke('gravar_wslconf', { distro, mudancas, reiniciar }),
    abrirPastaDeLogs: () => invoke('abrir_pasta_de_logs'),
    limitesDaMaquina: () => invoke('limites_da_maquina'),
  };
}

export async function conectar() {
  // Fora do app (navegador comum, durante o desenvolvimento da tela) não há backend,
  // e a tela roda sobre dados simulados com um aviso visível.
  api = dentroDoApp ? await conectarTauri() : criarDemonstracao();
  estado.demonstracao = !dentroDoApp;
  try {
    estado.receberDistros(await api.listarDistros());
    estado.receberEventos(await api.listarAtividade());
    const recursos = await api.estadoRecursos();
    estado.memoriaWindows = recursos.memoria.total_bytes ? recursos.memoria : null;
    estado.docker = recursos.docker;
    estado.saude = recursos.saude;
    estado.devolucao = recursos.devolucao;
    estado.devolverCacheAutomatico = recursos.devolver_cache_automatico;
    estado.gatilho = recursos.gatilho;
  } catch (e) {
    estado.avisar(String(e));
  }
}
