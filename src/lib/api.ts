import type { Amostra, Atualizacao, Distro, Evento, ResumoMemoria } from './tipos';
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
  procurarAtualizacao(): Promise<Atualizacao | null>;
  instalarAtualizacao(): Promise<void>;
  abrirPaginaDaVersao(versao: string): Promise<void>;
  lerLog(linhas: number): Promise<string[]>;
  abrirPastaDeLogs(): Promise<void>;
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
    procurarAtualizacao: () => invoke('procurar_atualizacao'),
    instalarAtualizacao: () => invoke('instalar_atualizacao'),
    abrirPaginaDaVersao: (versao) => invoke('abrir_pagina_da_versao', { versao }),
    lerLog: (linhas) => invoke('ler_log', { linhas }),
    abrirPastaDeLogs: () => invoke('abrir_pasta_de_logs'),
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
  } catch (e) {
    estado.avisar(String(e));
  }
}
