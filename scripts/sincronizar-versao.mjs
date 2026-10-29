// A versão do projeto mora no package.json. O tauri.conf.json já aponta para ele; este
// script copia o número para o Cargo.toml do workspace e para o Cargo.lock.
//
//   node scripts/sincronizar-versao.mjs                grava
//   node scripts/sincronizar-versao.mjs --conferir     falha se algo divergir
//   node scripts/sincronizar-versao.mjs --conferir v1.2.3   idem, e confere a tag

import { readFileSync, writeFileSync } from 'node:fs';

const MEMBROS = ['agente', 'protocolo', 'toca-do-pinguim'];

const versao = JSON.parse(readFileSync('package.json', 'utf8')).version;
const cargo = readFileSync('Cargo.toml', 'utf8');
const lock = readFileSync('Cargo.lock', 'utf8');

const linhaWorkspace = /(\[workspace\.package\][^[]*?\nversion = ")([^"]+)(")/;
const linhaLock = (nome) => new RegExp(`(\\[\\[package\\]\\]\\nname = "${nome}"\\nversion = ")([^"]+)(")`);

const [modo, tag] = process.argv.slice(2);

if (modo === '--conferir') {
  const erros = [];
  const noCargo = cargo.match(linhaWorkspace)?.[2];
  if (noCargo !== versao) erros.push(`Cargo.toml tem ${noCargo}, package.json tem ${versao}`);
  for (const nome of MEMBROS) {
    const noLock = lock.replace(/\r\n/g, '\n').match(linhaLock(nome))?.[2];
    if (noLock !== versao) erros.push(`Cargo.lock tem ${nome} ${noLock}, package.json tem ${versao}`);
  }
  if (tag && tag !== `v${versao}`) erros.push(`a tag ${tag} não corresponde à versão ${versao}`);
  if (erros.length) {
    console.error(`Versão dessincronizada:\n  ${erros.join('\n  ')}\nRode: node scripts/sincronizar-versao.mjs`);
    process.exit(1);
  }
  console.log(`Versão ${versao} igual em package.json, Cargo.toml e Cargo.lock${tag ? ` e na tag ${tag}` : ''}.`);
} else {
  if (!linhaWorkspace.test(cargo)) throw new Error('Cargo.toml sem version em [workspace.package]');
  writeFileSync('Cargo.toml', cargo.replace(linhaWorkspace, `$1${versao}$3`));
  const crlf = lock.includes('\r\n');
  let novo = lock.replace(/\r\n/g, '\n');
  for (const nome of MEMBROS) {
    if (!linhaLock(nome).test(novo)) throw new Error(`Cargo.lock sem o pacote ${nome}`);
    novo = novo.replace(linhaLock(nome), `$1${versao}$3`);
  }
  writeFileSync('Cargo.lock', crlf ? novo.replace(/\n/g, '\r\n') : novo);
  console.log(`Versão ${versao} copiada para Cargo.toml e Cargo.lock.`);
}
