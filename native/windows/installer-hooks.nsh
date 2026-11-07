!macro NSIS_HOOK_PREINSTALL
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
  ${If} ${FileExists} "$INSTDIR\drift-router.exe"
    ExecWait '"$INSTDIR\drift-router.exe" --uninstall-service' $0
    ${If} $0 != 0
      Abort "Driftnet could not stop its network service. Restart Windows and retry."
    ${EndIf}
    Sleep 2500
  ${EndIf}
!macroend
!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\drift-router.exe" --install-service' $0
  ${If} $0 != 0
    Abort "Driftnet could not install its network service. Run this installer again to repair it."
  ${EndIf}
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  ${If} ${FileExists} "$INSTDIR\drift-router.exe"
    ExecWait '"$INSTDIR\drift-router.exe" --uninstall-service' $0
    ${If} $0 != 0
      Abort "Driftnet could not remove its network service. Restart Windows and retry."
    ${EndIf}
    Sleep 2500
  ${EndIf}
!macroend
