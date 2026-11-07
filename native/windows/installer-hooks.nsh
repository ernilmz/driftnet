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
  nsExec::ExecToStack '"$0" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\driftnet-validate-install.ps1" -InstallDirectory "$INSTDIR" -Mode ${MODE}'
  Pop $0
  Pop $1
  System::Call 'kernel32::SetEnvironmentVariableW(w "PATH", w "$R8")'
  Pop $R8
  ${If} $0 != 0
    DetailPrint "$1"
    Abort "Driftnet requires a protected Program Files installation. Remove an unsafe older installation and retry."
  ${EndIf}
  !if "${MODE}" != "Validate"
    SetOutPath "$INSTDIR"
  !endif
!macroend
!macro DRIFT_IS_SHORTCUT_TARGET SHORTCUT TARGET
  ; Read the resolved target, not SLGP_RAWPATH (which can retain short names or
  ; environment syntax), then normalize both existing paths with Windows.
  Push $4
  Push $5
  Push $6
  StrCpy $5 0
  System::Call 'ole32::CoInitializeEx(p 0, i 2) i.r4'
  ${If} ${FileExists} "${SHORTCUT}"
    !insertmacro ComHlpr_CreateInProcInstance ${CLSID_ShellLink} ${IID_IShellLink} r0 ".r6"
    ${If} $0 P<> 0
      StrCpy $1 0
      ${IUnknown::QueryInterface} $0 '("${IID_IPersistFile}", .r1).r6'
      ${If} $1 P<> 0
        ${IPersistFile::Load} $1 '("${SHORTCUT}", ${STGM_READ}).r6'
        ${If} $6 = 0
          StrCpy $2 ""
          ${IShellLink::GetPath} $0 '(.r2, ${MAX_PATH}, 0, 0).r6'
          ${If} $6 >= 0
          ${AndIf} $2 != ""
            ExpandEnvStrings $2 "$2"
            ClearErrors
            GetFullPathName $2 "$2"
            ${IfNot} ${Errors}
              GetFullPathName $3 "${TARGET}"
              ${IfNot} ${Errors}
                ${If} $2 == $3
                  StrCpy $5 1
                ${Else}
                  ; Match existing short/long aliases as well. Absolute path
                  ; comparison above also works after uninstall deletes files.
                  ClearErrors
                  GetFullPathName /SHORT $2 "$2"
                  ${IfNot} ${Errors}
                    GetFullPathName /SHORT $3 "${TARGET}"
                    ${IfNot} ${Errors}
                    ${AndIf} $2 == $3
                      StrCpy $5 1
                    ${EndIf}
                  ${EndIf}
                ${EndIf}
              ${EndIf}
            ${EndIf}
          ${EndIf}
        ${EndIf}
        ${IUnknown::Release} $1 '.r6'
      ${EndIf}
      ${IUnknown::Release} $0 '.r6'
    ${EndIf}
  ${EndIf}
  ${If} $4 = 0
  ${OrIf} $4 = 1
    System::Call 'ole32::CoUninitialize()'
  ${EndIf}
  StrCpy $0 $5
  Pop $6
  Pop $5
  Pop $4
  Push $0
!macroend
!macro DRIFT_DELETE_MATCHING_SHORTCUT SHORTCUT
  !insertmacro DRIFT_IS_SHORTCUT_TARGET "${SHORTCUT}" "$INSTDIR\drift-desktop.exe"
  Pop $0
  ${If} $0 = 1
    Delete "${SHORTCUT}"
  ${EndIf}
!macroend
!macro NSIS_HOOK_PREINSTALL
  !insertmacro DRIFT_VALIDATE_INSTALL Prepare
  ; Remove only the former product shortcuts that target this exact installed
  ; executable. Preserve unrelated shortcuts and all user configuration.
  !insertmacro DRIFT_DELETE_MATCHING_SHORTCUT "$SMPROGRAMS\Drift\Drift.lnk"
  RMDir "$SMPROGRAMS\Drift"
  !insertmacro DRIFT_DELETE_MATCHING_SHORTCUT "$DESKTOP\Drift.lnk"
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
