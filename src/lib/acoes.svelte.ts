// Ações que mudam o estado do WSL, usadas pela aba Distros e pela busca. As que
// encerram processos em massa passam pela confirmação global.

import { api } from './api';
import { estado } from './estado.svelte';
import { ativo, comProblema, type Projeto } from './projetos';
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

// Com o WSL travado, o desligamento comum espera o que travou e não volta (medido: 180 s); o
// forçado derruba a VM na hora (medido: 1 s). Sem trava, vai o comum, e o forçado só é oferecido
// se ele não voltar no prazo.
const TEXTO_FORCAR =
  'O desligamento forçado derruba a VM do WSL sem esperar nada terminar: todas as distros e todos os containers param na hora, e o que estava só na memória deles se perde. Se o Docker Desktop não voltar sozinho, abra-o de novo.';

function pedirForca(titulo: string, motivo: string) {
  estado.pedido = {
    titulo,
    texto: `${motivo} ${TEXTO_FORCAR}`,
    acao: 'Forçar o desligamento',
    executar: () =>
      executar('WSL', 'Forçando', async () => {
        const docker = estado.docker.ligado;
        await api.desligarWsl(true);
        // O Docker Desktop aberto costuma religar a distro dele sozinho (medido: 17 s); o
        // fechado, não. O aviso some quando ele volta.
        if (docker) estado.dockerDerrubadoMs = Date.now();
      }),
  };
}

export function desligarWsl() {
  if (estado.saude) {
    pedirForca('Forçar o desligamento do WSL?', 'O WSL não está respondendo, e o desligamento comum fica esperando o que travou.');
    return;
  }
  const ligadas = estado.distros.filter((d) => d.rodando).map((d) => d.nome);
  estado.pedido = {
    titulo: 'Desligar o WSL?',
    texto: `Todas as distros são terminadas (${ligadas.join(', ') || 'nenhuma ligada agora'}) e a VM do WSL é desligada, liberando a memória dela no Windows.`,
    acao: 'Desligar',
    executar: async () => {
      let desligou = true;
      await executar('WSL', 'Desligando', async () => {
        desligou = await api.desligarWsl(false);
      });
      if (!desligou) pedirForca('O WSL não desligou. Forçar?', 'O desligamento comum não voltou em 60 s: o WSL está travado.');
    },
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

function descrever(p: Projeto): string {
  return p.avulso ? `o container ${p.nome}` : `o projeto ${p.nome}`;
}

export function pararProjeto(p: Projeto) {
  const ids = p.conteineres.filter(ativo).map((c) => c.id);
  const quantos = ids.length === 1 ? '1 container para e fica parado' : `${ids.length} containers param e ficam parados`;
  estado.pedido = {
    titulo: `Parar ${p.nome}?`,
    texto: `${quantos} até você ligar de novo, aqui ou pelo Docker; os que usam restart: always também voltam quando o Docker reinicia. Nada é apagado: imagens, volumes e dados continuam lá. A memória que eles usavam fica livre na VM do WSL.`,
    acao: 'Parar',
    executar: () => executar(`docker:${p.chave}`, 'Parando', () => api.pararConteineres(ids, descrever(p))),
  };
}

/** Reinicia só os containers do projeto que estão com problema. */
export function reiniciarComProblema(p: Projeto) {
  const alvos = p.conteineres.filter(comProblema);
  const nomes = alvos.map((c) => c.nome).join(', ');
  estado.pedido = {
    titulo: `Reiniciar ${alvos.length === 1 ? nomes : `${alvos.length} containers`}?`,
    texto:
      alvos.length === 1
        ? `${nomes} é parado e ligado de novo, com a mesma configuração e os mesmos dados: nada é apagado. O que estava só na memória dele se perde.`
        : `${nomes} são parados e ligados de novo, com a mesma configuração e os mesmos dados: nada é apagado. O que estava só na memória deles se perde.`,
    acao: 'Reiniciar',
    executar: () =>
      executar(`docker:${p.chave}`, 'Reiniciando', () => api.reiniciarConteineres(alvos.map((c) => c.id), alvos.length === 1 ? `o container ${nomes}` : `${alvos.length} containers de ${p.nome}`)),
  };
}

export function ligarProjeto(p: Projeto) {
  const ids = p.conteineres.map((c) => c.id);
  executar(`docker:${p.chave}`, 'Ligando', () => api.iniciarConteineres(ids, descrever(p)));
}
