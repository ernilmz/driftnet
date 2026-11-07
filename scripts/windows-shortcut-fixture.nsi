Unicode true
!include LogicLib.nsh
!include FileFunc.nsh
!include x64.nsh
!include "${__FILEDIR__}\..\native\windows\installer-hooks.nsh"
Name "Driftnet shortcut fixture"
OutFile "${__FILEDIR__}\..\target\Driftnet-Shortcut-Fixture.exe"
RequestExecutionLevel admin
InstallDir "$PROGRAMFILES64\Drift"
SilentInstall silent
Function FixtureFailure
  FileOpen $2 "$INSTDIR\fixture-error.txt" w
  FileWrite $2 "$0 : $1$\r$\n"
  FileClose $2
  SetErrorLevel 61
  Quit
FunctionEnd
Section
  SetShellVarContext all
  SetOutPath "$SYSDIR"
  !insertmacro DRIFT_RUN_VALIDATOR CleanupLegacy "-Verbose"
  ${If} $0 != 0
    Call FixtureFailure
  ${EndIf}
  ${If} ${FileExists} "$SMPROGRAMS\Drift\Drift.lnk"
    StrCpy $1 "Legacy shortcut survived the post-install stage: $1"
    Call FixtureFailure
  ${EndIf}
  ${IfNot} ${FileExists} "$SMPROGRAMS\Driftnet\Driftnet.lnk"
    StrCpy $1 "Current shortcut was removed during legacy cleanup"
    Call FixtureFailure
  ${EndIf}
  !insertmacro DRIFT_IS_SHORTCUT_TARGET "$SMPROGRAMS\Driftnet\Driftnet.lnk" "$INSTDIR\drift-desktop.exe"
  Pop $0
  ${If} $0 != 1
    StrCpy $1 "Current shortcut exact comparison failed"
    Call FixtureFailure
  ${EndIf}
  !insertmacro DRIFT_IS_SHORTCUT_TARGET "$SMPROGRAMS\Driftnet\Unrelated.lnk" "$INSTDIR\drift-desktop.exe"
  Pop $0
  ${If} $0 != 0
    StrCpy $1 "An unrelated target incorrectly matched"
    Call FixtureFailure
  ${EndIf}
  !insertmacro DRIFT_RUN_VALIDATOR CleanupAll '-StartMenuFolder "Driftnet"'
  ${If} $0 != 0
    Call FixtureFailure
  ${EndIf}
  SetErrorLevel 0
SectionEnd
