//! Log do kernel (`/dev/kmsg`) a partir do momento em que o agente sobe.
//!
//! Interessa uma coisa só por enquanto: o kernel encerrando processo por falta de memória.
//! Sem isso, um processo some da lista e ninguém sabe por quê.

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::io::AsRawFd;

use protocolo::EventoKernel;

pub struct Kmsg {
    arquivo: File,
}

impl Kmsg {
    pub fn abrir() -> Option<Kmsg> {
        let arquivo = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/kmsg")
            .ok()?;
        // SEEK_END pula o que já estava no buffer: o que aconteceu antes do agente
        // não é novidade e se repetiria a cada reconexão.
        // SAFETY: lseek num descritor válido e aberto acima.
        unsafe { libc::lseek(arquivo.as_raw_fd(), 0, libc::SEEK_END) };
        Some(Kmsg { arquivo })
    }

    /// Cada `read` em `/dev/kmsg` devolve exatamente um registro, até não haver mais.
    pub fn novos(&mut self) -> Vec<EventoKernel> {
        let mut eventos = Vec::new();
        let mut buf = vec![0u8; 8192];
        loop {
            match self.arquivo.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(e) = interpretar(&String::from_utf8_lossy(&buf[..n])) {
                        eventos.push(e);
                    }
                }
                // EPIPE: registros sobrescritos antes da leitura; o próximo `read` segue.
                Err(e) if e.raw_os_error() == Some(libc::EPIPE) => continue,
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        eventos
    }
}

/// Registro: `prioridade,seq,microssegundos,flags;mensagem` seguido de linhas de contexto.
fn interpretar(registro: &str) -> Option<EventoKernel> {
    let (cabecalho, resto) = registro.split_once(';')?;
    let mensagem = resto.lines().next()?.trim();
    // "Out of memory: Killed process 123 (node)" e a variante de cgroup terminam iguais.
    if !mensagem.contains("Killed process") {
        return None;
    }
    let seq = cabecalho.split(',').nth(1)?.parse().ok()?;
    Some(EventoKernel { seq, texto: mensagem.to_string() })
}
