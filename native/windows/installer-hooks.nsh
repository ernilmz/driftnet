!include "${__FILEDIR__}\validator-embedded.nsh"
!macro DRIFT_VALIDATE_INSTALL MODE
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  !insertmacro DRIFT_WRITE_VALIDATOR
  ${If} ${RunningX64}
    StrCpy $0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${Else}
    StrCpy $0 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}
  nsExec::ExecToStack '"$0" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\driftnet-validate-install.ps1" -InstallDirectory "$INSTDIR" -Mode ${MODE}'
  Pop $0
  Pop $1
  ${If} $0 != 0
    DetailPrint "$1"
    Abort "Driftnet requires a protected Program Files installation. Remove an unsafe older installation and retry."
  ${EndIf}
  SetOutPath "$INSTDIR"
!macroend
!macro NSIS_HOOK_PREINSTALL
  !insertmacro DRIFT_VALIDATE_INSTALL Prepare
  ; Remove only the former product shortcuts that target this exact installed
  ; executable. Preserve unrelated shortcuts and all user configuration.
  !insertmacro IsShortcutTarget "$SMPROGRAMS\Drift\Drift.lnk" "$INSTDIR\drift-desktop.exe"
  Pop $0
  ${If} $0 = 1
    Delete "$SMPROGRAMS\Drift\Drift.lnk"
    RMDir "$SMPROGRAMS\Drift"
  ${EndIf}
  !insertmacro IsShortcutTarget "$DESKTOP\Drift.lnk" "$INSTDIR\drift-desktop.exe"
  Pop $0
  ${If} $0 = 1
    Delete "$DESKTOP\Drift.lnk"
  ${EndIf}
!macroend
!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\drift-router.exe" --install-service' $0
  ${If} $0 != 0
    Abort "Driftnet could not install its network service. Run this installer again to repair it."
  ${EndIf}
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro DRIFT_VALIDATE_INSTALL Remove
!macroend
