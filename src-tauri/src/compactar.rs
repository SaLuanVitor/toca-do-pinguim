//! Compactar um disco virtual devolve ao Windows o espaço que o Linux já liberou por dentro.
//!
//! O `.vhdx` dinâmico cresce e não encolhe sozinho: medido numa máquina, o do Ubuntu tinha
//! 112 GB no Windows com 90 GB ocupados por dentro. O Windows 11 Home não tem `Optimize-VHD`,
//! e o `diskpart` só compacta como administrador e com o disco solto, então o WSL inteiro é
//! desligado antes e o Windows pede a permissão.

use std::path::Path;
use std::time::Duration;

use log::{info, warn};

/// Compactar dezenas de GB num disco lento leva vários minutos.
const PRAZO: Duration = Duration::from_secs(60 * 60);
/// O Windows devolve 1223 quando a pessoa recusa o pedido de administrador.
const ERRO_CANCELADO: u32 = 1223;
/// O `--shutdown` volta antes de a VM soltar os discos.
const PRAZO_SOLTAR: Duration = Duration::from_secs(60);
/// Sem distro ligada por este tempo, o que ainda prende o arquivo é um anexo do diskpart, que
/// o próprio roteiro desfaz.
const SEM_DISTRO: Duration = Duration::from_secs(10);

/// O roteiro vai pela entrada padrão, não por `/s`: com `/s` o diskpart para no primeiro erro,
/// e um compact que falhava deixava o disco anexado, sem o WSL conseguir abri-lo (medido). Pela
/// entrada ele segue até o fim, e o detach roda sempre. Um attach que falha porque o disco já
/// estava anexado também não atrapalha: o compact e o detach valem para ele do mesmo jeito.
pub fn script(caminho: &str) -> String {
    format!("select vdisk file=\"{caminho}\"\r\nattach vdisk readonly\r\ncompact vdisk\r\ndetach vdisk\r\nexit\r\n")
}

/// O Docker Desktop religa o WSL logo depois do `--shutdown` e, com a integração ligada,
/// sobe as distros também; com ele aberto, o disco fica preso.
pub fn docker_aberto() -> bool {
    ["Docker Desktop.exe", "com.docker.backend.exe"].iter().any(|p| crate::recursos::processo_rodando(p))
}

/// Devolve o tamanho do arquivo antes e depois.
pub fn compactar(caminho: &Path) -> Result<(u64, u64), String> {
    let texto = caminho.to_str().filter(|t| !t.contains('"')).ok_or("caminho do disco inválido")?;
    let tamanho = || std::fs::metadata(caminho).map(|m| m.len()).map_err(|e| format!("{texto}: {e}"));
    let antes = tamanho()?;
    esperar_solto(caminho)?;
    let pasta = std::env::temp_dir().join(format!("toca-do-pinguim-compactar-{}", std::process::id()));
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    let roteiro = pasta.join("compactar.txt");
    let registro = pasta.join("diskpart.log");
    std::fs::write(&roteiro, script(texto)).map_err(|e| e.to_string())?;
    let parametros = format!("/c diskpart < \"{}\" > \"{}\" 2>&1", roteiro.display(), registro.display());
    let codigo = como_administrador(&parametros, PRAZO);
    let saida = std::fs::read(&registro).map(|b| decodificar_oem(&b)).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&pasta);
    let codigo = codigo?;
    let depois = tamanho()?;
    info!("compactar {texto}: código {codigo}, de {antes} para {depois} bytes");
    // Pela entrada padrão o diskpart sai com 0 mesmo quando um comando falha; o que prova a
    // compactação é o arquivo ter diminuído.
    if depois >= antes {
        warn!("diskpart: {saida}");
        return Err(format!("o disco não diminuiu; o diskpart disse: {}", ultimas_linhas(&saida, 4)));
    }
    Ok((antes, depois))
}

/// Espera o disco ficar solto antes de pedir a permissão de administrador. Medido: numa
/// tentativa sem esta espera, uma VM nova subiu 4 s depois do `--shutdown`, e o diskpart achou
/// o arquivo em uso. Recusa só se uma distro religou; arquivo preso sem distro ligada é anexo
/// de uma tentativa anterior, e o roteiro o solta.
fn esperar_solto(caminho: &Path) -> Result<(), String> {
    use std::os::windows::fs::OpenOptionsExt;
    let inicio = std::time::Instant::now();
    let mut sem_distro_desde: Option<std::time::Instant> = None;
    loop {
        if std::fs::OpenOptions::new().read(true).share_mode(0).open(caminho).is_ok() {
            return Ok(());
        }
        let ligadas = crate::wsl::rodando();
        if ligadas.is_empty() {
            let desde = *sem_distro_desde.get_or_insert_with(std::time::Instant::now);
            if desde.elapsed() >= SEM_DISTRO {
                warn!("disco preso sem distro ligada; segue para o diskpart, que solta o anexo");
                return Ok(());
            }
        } else {
            sem_distro_desde = None;
        }
        if inicio.elapsed() >= PRAZO_SOLTAR {
            return Err(format!(
                "{} ligou de novo depois do desligamento, e o disco ficou em uso; feche o programa que a abriu (terminal, editor) e tente de novo",
                ligadas.join(", ")
            ));
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn ultimas_linhas(texto: &str, n: usize) -> String {
    let linhas: Vec<&str> = texto.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    linhas[linhas.len().saturating_sub(n)..].join(" ")
}

/// Roda `cmd.exe` com os parâmetros como administrador, sem janela, e espera o fim.
fn como_administrador(parametros: &str, prazo: Duration) -> Result<u32, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, TerminateProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let largo = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (verbo, arquivo, parametros) = (largo("runas"), largo("cmd.exe"), largo(parametros));
    // SAFETY: a estrutura é zerada e preenchida com ponteiros para buffers que vivem até o fim
    // da função; o handle devolvido é fechado em todos os caminhos.
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verbo.as_ptr();
        info.lpFile = arquivo.as_ptr();
        info.lpParameters = parametros.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            let erro = GetLastError();
            return Err(if erro == ERRO_CANCELADO {
                "a permissão de administrador foi recusada; o disco não foi compactado".to_string()
            } else {
                format!("não foi possível pedir a permissão de administrador (erro {erro})")
            });
        }
        if info.hProcess.is_null() {
            return Err("o Windows não devolveu o processo do diskpart".to_string());
        }
        let espera = WaitForSingleObject(info.hProcess, prazo.as_millis().min(u32::MAX as u128 - 1) as u32);
        if espera != WAIT_OBJECT_0 {
            TerminateProcess(info.hProcess, 1);
            CloseHandle(info.hProcess);
            return Err(format!("o diskpart não terminou em {} min", prazo.as_secs() / 60));
        }
        let mut codigo = 0u32;
        GetExitCodeProcess(info.hProcess, &mut codigo);
        CloseHandle(info.hProcess);
        Ok(codigo)
    }
}

/// O `diskpart` escreve na página de código OEM do console (850 no Windows em português).
fn decodificar_oem(bytes: &[u8]) -> String {
    use windows_sys::Win32::Globalization::{CP_OEMCP, MultiByteToWideChar};
    if bytes.is_empty() {
        return String::new();
    }
    // SAFETY: a primeira chamada mede, a segunda escreve no buffer do tamanho medido.
    unsafe {
        let n = MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if n <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut largo = vec![0u16; n as usize];
        MultiByteToWideChar(CP_OEMCP, 0, bytes.as_ptr(), bytes.len() as i32, largo.as_mut_ptr(), n);
        String::from_utf16_lossy(&largo)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn roteiro_anexa_compacta_e_solta() {
        let s = script(r"C:\Users\voce\ext4.vhdx");
        let linhas: Vec<&str> = s.lines().collect();
        assert_eq!(linhas[0], r#"select vdisk file="C:\Users\voce\ext4.vhdx""#);
        assert_eq!(linhas[1], "attach vdisk readonly");
        // O compact do diskpart não aceita parâmetro: com `noerr` ele recusa o comando.
        assert_eq!(linhas[2], "compact vdisk");
        assert_eq!(linhas[3], "detach vdisk");
    }

    #[test]
    fn caminho_com_aspas_e_recusado() {
        assert!(compactar(Path::new(r#"C:\a"b.vhdx"#)).is_err());
    }

    #[test]
    fn ultimas_linhas_pula_as_vazias() {
        assert_eq!(ultimas_linhas("a\n\n b \nc\n\n", 2), "b c");
    }

    #[test]
    fn oem_decodifica_acento() {
        // "ê" na página 850 é 0x88; na 437, também é "ê".
        assert_eq!(decodificar_oem(&[b'v', 0x88]), "vê");
    }
}
