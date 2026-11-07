!macro NSIS_HOOK_PREINSTALL
  ${If} ${FileExists} "$INSTDIR\drift-router.exe"
    ExecWait '"$INSTDIR\drift-router.exe" --uninstall-service' $0
    ${If} $0 != 0
      Abort "Drift could not stop its network service. Restart Windows and retry."
    ${EndIf}
    Sleep 2500
  ${EndIf}
!macroend
!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\drift-router.exe" --install-service' $0
  ${If} $0 != 0
    Abort "Drift could not install its network service. Run this installer again to repair it."
  ${EndIf}
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  ${If} ${FileExists} "$INSTDIR\drift-router.exe"
    ExecWait '"$INSTDIR\drift-router.exe" --uninstall-service' $0
    ${If} $0 != 0
      Abort "Drift could not remove its network service. Restart Windows and retry."
    ${EndIf}
    Sleep 2500
  ${EndIf}
!macroend
