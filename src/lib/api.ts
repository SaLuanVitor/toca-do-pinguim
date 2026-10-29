import type { Amostra, Distro } from './tipos';
import { estado } from './estado.svelte';
import { criarDemonstracao } from './demonstracao';

export interface Api {
  listarDistros(): Promise<Distro[]>;
  iniciarDistro(nome: string): Promise<void>;
  terminarDistro(nome: string): Promise<void>;
  reiniciarDistro(nome: string): Promise<void>;
  desligarWsl(): Promise<void>;
  enviarSinal(distro: string, pid: number, sinal: number): Promise<void>;
  definirIntervalo(ms: number): Promise<void>;
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
  await listen<{ distro: string | null; mensagem: string }>('erro', (e) =>
    estado.avisar(e.payload.distro ? `${e.payload.distro}: ${e.payload.mensagem}` : e.payload.mensagem),
  );

  return {
    listarDistros: () => invoke('listar_distros'),
    iniciarDistro: (nome) => invoke('iniciar_distro', { nome }),
    terminarDistro: (nome) => invoke('terminar_distro', { nome }),
    reiniciarDistro: (nome) => invoke('reiniciar_distro', { nome }),
    desligarWsl: () => invoke('desligar_wsl'),
    enviarSinal: (distro, pid, sinal) => invoke('enviar_sinal', { distro, pid, sinal }),
    definirIntervalo: (ms) => invoke('definir_intervalo', { ms }),
  };
}

export async function conectar() {
  // Fora do app (navegador comum, durante o desenvolvimento da tela) não há backend,
  // e a tela roda sobre dados simulados com um aviso visível.
  api = dentroDoApp ? await conectarTauri() : criarDemonstracao();
  estado.demonstracao = !dentroDoApp;
  try {
    estado.receberDistros(await api.listarDistros());
  } catch (e) {
    estado.avisar(String(e));
  }
}
