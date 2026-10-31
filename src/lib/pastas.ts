// Organiza o resultado da análise de pastas em grupos com nome e explicação, para a lista ler
// igual em qualquer distro: o que é programa, o que é arquivo seu, o que é dado de serviço. No
// disco do Docker, os grupos são os dele: volumes, imagens, cache de build.

import type { AnalisePastas, Pasta } from './tipos';

export interface ItemPasta {
  /** Nome curto para a lista; o caminho inteiro fica em `caminho`. */
  nome: string;
  caminho: string;
  bytes: number;
  descricao?: string;
}

export interface GrupoPastas {
  titulo: string;
  descricao: string;
  bytes: number;
  itens: ItemPasta[];
}

const DISCO_DOCKER = '/mnt/docker-desktop-disk/';

interface Regra {
  titulo: string;
  descricao: string;
  /** Pasta da raiz e o que ela guarda. */
  pastas: Record<string, string>;
}

const REGRAS_DA_RAIZ: Regra[] = [
  {
    titulo: 'Seus arquivos',
    descricao: 'Projetos, downloads, ambientes e caches de quem usa a distro.',
    pastas: { '/home': 'Pastas dos usuários', '/root': 'Pasta do usuário root' },
  },
  {
    titulo: 'Programas',
    descricao: 'O que foi instalado pelo gerenciador de pacotes ou à mão. Diminui desinstalando.',
    pastas: {
      '/usr': 'Programas e bibliotecas do sistema',
      '/opt': 'Programas instalados à parte',
      '/snap': 'Pacotes snap',
      '/nix': 'Pacotes do Nix',
      '/bin': 'Comandos básicos',
      '/sbin': 'Comandos de administração',
      '/lib': 'Bibliotecas do sistema',
      '/lib32': 'Bibliotecas de 32 bits',
      '/lib64': 'Bibliotecas de 64 bits',
      '/libx32': 'Bibliotecas x32',
    },
  },
  {
    titulo: 'Dados do sistema e de serviços',
    descricao: 'Logs, cache de pacotes, bancos de dados e o que os serviços guardam.',
    pastas: { '/var': 'Logs, caches, bancos e dados de serviços', '/srv': 'Dados servidos pela distro' },
  },
  {
    titulo: 'Temporários',
    descricao: 'No WSL, o /tmp não se esvazia nem ao reiniciar; a lista do /tmp, abaixo, mostra o que dá para apagar.',
    pastas: { '/tmp': 'Arquivos temporários' },
  },
];

const OUTROS_DA_RAIZ = { titulo: 'Configuração e outros', descricao: 'Configuração, inicialização e pastas fora do padrão.' };
const DESCRICOES_SOLTAS: Record<string, string> = { '/etc': 'Configuração', '/boot': 'Inicialização' };

interface RegraDocker {
  titulo: string;
  descricao: string;
  casa: (relativo: string) => boolean;
  nome: (relativo: string) => string;
}

const REGRAS_DO_DOCKER: RegraDocker[] = [
  {
    titulo: 'Volumes',
    descricao: 'Dados que os containers guardam fora da imagem, como bancos. Os sem uso se apagam em Espaço do Docker.',
    casa: (r) => r.startsWith('data/docker/volumes/'),
    nome: (r) => r.slice('data/docker/volumes/'.length),
  },
  {
    titulo: 'Imagens',
    descricao: 'As imagens e as camadas delas, guardadas pelo containerd. Diminui apagando imagens sem uso.',
    casa: (r) => r.startsWith('data/desktop-containerd/'),
    nome: (r) => r.slice('data/'.length),
  },
  {
    titulo: 'Cache de build',
    descricao: 'Camadas guardadas para builds futuros. Diminui apagando o cache em Espaço do Docker.',
    casa: (r) => r.startsWith('data/docker/buildkit'),
    nome: (r) => r.slice('data/docker/'.length),
  },
  {
    titulo: 'Containers',
    descricao: 'Logs e configuração de cada container.',
    casa: (r) => r.startsWith('data/docker/containers'),
    nome: (r) => r.slice('data/docker/'.length),
  },
  {
    titulo: 'Arquivos do próprio Docker Desktop',
    descricao: 'Imagens de sistema que o Docker Desktop usa para subir; ele mesmo as gerencia.',
    casa: (r) => r.startsWith('isocache/'),
    nome: (r) => r.slice('isocache/entries/'.length) || r,
  },
];

const OUTROS_DO_DOCKER = { titulo: 'Outros', descricao: 'Demais pastas do disco de dados do Docker.' };

function agrupar<T>(itens: T[], chave: (i: T) => { titulo: string; descricao: string }, comoItem: (i: T) => ItemPasta): GrupoPastas[] {
  const grupos = new Map<string, GrupoPastas>();
  for (const i of itens) {
    const { titulo, descricao } = chave(i);
    const g = grupos.get(titulo) ?? { titulo, descricao, bytes: 0, itens: [] };
    const item = comoItem(i);
    g.bytes += item.bytes;
    g.itens.push(item);
    grupos.set(titulo, g);
  }
  const lista = [...grupos.values()].filter((g) => g.bytes > 0);
  for (const g of lista) g.itens.sort((a, b) => b.bytes - a.bytes);
  return lista.sort((a, b) => b.bytes - a.bytes);
}

function daRaiz(p: Pasta): ItemPasta {
  const regra = REGRAS_DA_RAIZ.find((r) => p.caminho in r.pastas);
  return { nome: p.caminho, caminho: p.caminho, bytes: p.bytes, descricao: regra?.pastas[p.caminho] ?? DESCRICOES_SOLTAS[p.caminho] };
}

function doDocker(p: Pasta): ItemPasta {
  const relativo = p.caminho.startsWith(DISCO_DOCKER) ? p.caminho.slice(DISCO_DOCKER.length) : p.caminho;
  const regra = REGRAS_DO_DOCKER.find((r) => r.casa(relativo));
  return { nome: regra ? regra.nome(relativo) : relativo, caminho: p.caminho, bytes: p.bytes };
}

export function organizar(analise: AnalisePastas): GrupoPastas[] {
  if (analise.so_leitura) {
    return agrupar(
      analise.raiz,
      (p) => {
        const relativo = p.caminho.startsWith(DISCO_DOCKER) ? p.caminho.slice(DISCO_DOCKER.length) : p.caminho;
        return REGRAS_DO_DOCKER.find((r) => r.casa(relativo)) ?? OUTROS_DO_DOCKER;
      },
      doDocker,
    );
  }
  return agrupar(analise.raiz, (p) => REGRAS_DA_RAIZ.find((r) => p.caminho in r.pastas) ?? OUTROS_DA_RAIZ, daRaiz);
}
