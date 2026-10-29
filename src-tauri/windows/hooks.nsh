; Ganchos do instalador NSIS do Tauri (bundle.windows.nsis.installerHooks).
; A instalação é por usuário, então tudo vai em HKCU e nada pede administrador.

!define TOCA_APP_PATHS "Software\Microsoft\Windows\CurrentVersion\App Paths\toca-do-pinguim.exe"
!define TOCA_UNINSTALL "Software\Microsoft\Windows\CurrentVersion\Uninstall\Toca do Pinguim"

!macro NSIS_HOOK_POSTINSTALL
  ; "toca-do-pinguim" passa a abrir o app pelo Win+R, pelo `start` do cmd e pelo
  ; Start-Process do PowerShell, sem mexer no PATH.
  WriteRegStr HKCU "${TOCA_APP_PATHS}" "" "$INSTDIR\toca-do-pinguim.exe"
  WriteRegStr HKCU "${TOCA_APP_PATHS}" "Path" "$INSTDIR"
  ; Comando de remoção sem perguntas, para scripts e ferramentas de inventário.
  WriteRegStr HKCU "${TOCA_UNINSTALL}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "${TOCA_APP_PATHS}"
!macroend
