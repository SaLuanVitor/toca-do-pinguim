// Ações que mudam o estado do WSL, usadas pela aba Distros e pela busca. As que
// encerram processos em massa passam pela confirmação global.

import { api } from './api';
import { estado } from './estado.svelte';
import type { Distro } from './tipos';

export const ocupadas = $state<Record<string, string>>({});

async function executar(nome: string, rotulo: string, acao: () => Promise<void>) {
  ocupadas[nome] = rotulo;
  try {
    await acao();
    estado.receberDistros(await api.listarDistros());
  } catch (e) {
    estado.avisar(`${nome}: ${e}`);
  } finally {
    delete ocupadas[nome];
  }
}

export function iniciar(d: Distro) {
  executar(d.nome, 'Iniciando', () => api.iniciarDistro(d.nome));
}

export function terminar(d: Distro) {
  estado.pedido = {
    titulo: `Terminar ${d.nome}?`,
    texto: `Todos os processos de ${d.nome} serão encerrados sem aviso a eles. O que não estiver salvo dentro da distro se perde.`,
    acao: 'Terminar',
    executar: () => executar(d.nome, 'Terminando', () => api.terminarDistro(d.nome)),
  };
}

export function reiniciar(d: Distro) {
  estado.pedido = {
    titulo: `Reiniciar ${d.nome}?`,
    texto: `${d.nome} é terminada e ligada de novo. Todos os processos dela são encerrados.`,
    acao: 'Reiniciar',
    executar: () => executar(d.nome, 'Reiniciando', () => api.reiniciarDistro(d.nome)),
  };
}

export function desligarWsl() {
  const ligadas = estado.distros.filter((d) => d.rodando).map((d) => d.nome);
  estado.pedido = {
    titulo: 'Desligar o WSL?',
    texto: `Todas as distros são terminadas (${ligadas.join(', ') || 'nenhuma ligada agora'}) e a VM do WSL é desligada, liberando a memória dela no Windows.`,
    acao: 'Desligar',
    executar: () => executar('WSL', 'Desligando', () => api.desligarWsl()),
  };
}

export async function encerrarProcesso(distro: string, pid: number, nome: string, sinal: number) {
  if (pid <= 1) return;
  try {
    await api.enviarSinal(distro, pid, sinal, nome);
  } catch (e) {
    estado.avisar(String(e));
  }
}
