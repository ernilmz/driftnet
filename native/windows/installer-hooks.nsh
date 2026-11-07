!include "${__FILEDIR__}\validator-embedded.nsh"
!macro DRIFT_RUN_VALIDATOR MODE EXTRA
  Push $R7
  StrCpy $R7 $OUTDIR
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  !insertmacro DRIFT_WRITE_VALIDATOR
  ${If} ${RunningX64}
    StrCpy $0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${Else}
    StrCpy $0 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}
  ; Launch the trusted OS shell from an OS directory, without inherited .NET
  ; profiler/AppDomain injection or user-module search paths.
  SetOutPath "$SYSDIR"
  Push $R8
  ReadEnvStr $R8 "PATH"
  System::Call 'kernel32::SetEnvironmentVariableW(w "PATH", w "$SYSDIR;$WINDIR")'
  System::Call 'kernel32::SetEnvironmentVariableW(w "COR_ENABLE_PROFILING", w "0")'
  System::Call 'kernel32::SetEnvironmentVariableW(w "COR_PROFILER", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "COR_PROFILER_PATH", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "COR_PROFILER_PATH_32", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "COR_PROFILER_PATH_64", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "APPDOMAIN_MANAGER_ASM", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "APPDOMAIN_MANAGER_TYPE", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "COMPLUS_InstallRoot", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "DEVPATH", p 0)'
  System::Call 'kernel32::SetEnvironmentVariableW(w "PSModulePath", p 0)'
  nsExec::ExecToStack '"$0" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\driftnet-validate-install.ps1" -InstallDirectory "$INSTDIR" -Mode ${MODE} ${EXTRA}'
  Pop $0
  Pop $1
  System::Call 'kernel32::SetEnvironmentVariableW(w "PATH", w "$R8")'
  Pop $R8
  SetOutPath "$R7"
  Pop $R7
!macroend
!macro DRIFT_VALIDATE_INSTALL MODE
  !insertmacro DRIFT_RUN_VALIDATOR ${MODE} ""
  ${If} $0 != 0
    DetailPrint "$1"
    Abort "Driftnet requires a protected Program Files installation. Remove an unsafe older installation and retry."
  ${EndIf}
!macroend
!macro DRIFT_IS_SHORTCUT_TARGET SHORTCUT TARGET
  !insertmacro DRIFT_RUN_VALIDATOR Shortcut '-ShortcutPath "${SHORTCUT}" -ShortcutTarget "${TARGET}"'
  ${If} $0 = 0
    StrCpy $0 1
  ${Else}
    StrCpy $0 0
  ${EndIf}
  Push $0
!macroend
!macro NSIS_HOOK_PREINSTALL
  !insertmacro DRIFT_VALIDATE_INSTALL Prepare
!macroend
!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\drift-router.exe" --install-service' $0
  ${If} $0 != 0
    Abort "Driftnet could not install its network service. Run this installer again to repair it."
  ${EndIf}
  !insertmacro DRIFT_VALIDATE_INSTALL CleanupLegacy
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  SetOutPath "$SYSDIR"
  !insertmacro MUI_STARTMENU_GETFOLDER Application $AppStartMenuFolder
  !insertmacro DRIFT_RUN_VALIDATOR CleanupAll '-StartMenuFolder "$AppStartMenuFolder"'
  ${If} $0 != 0
    Abort "Driftnet could not safely remove its shortcuts."
  ${EndIf}
  !insertmacro DRIFT_VALIDATE_INSTALL Remove
!macroend
