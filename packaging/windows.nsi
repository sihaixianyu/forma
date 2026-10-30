Unicode true
!include "MUI2.nsh"
!include "x64.nsh"

Name "Forma"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Forma"
InstallDirRegKey HKCU "Software\Forma" "InstallDir"
RequestExecutionLevel user
ManifestDPIAware true

!define MUI_ABORTWARNING
!define MUI_ICON "${STAGE}\forma.ico"
!define MUI_UNICON "${STAGE}\forma.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\forma.exe"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "Forma requires 64-bit Windows."
    Abort
  ${EndIf}
  SetShellVarContext current
FunctionEnd

Section "Forma"
  SetOutPath "$INSTDIR"
  File "${STAGE}\forma.exe"
  File "${STAGE}\LICENSE"
  File "${STAGE}\FONT-LICENSE.txt"
  File "${STAGE}\JETBRAINS-LICENSE.txt"
  File "${STAGE}\INTER-LICENSE.txt"
  File "${STAGE}\PIXELIFY-LICENSE.txt"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Forma"
  CreateShortcut "$SMPROGRAMS\Forma\Forma.lnk" "$INSTDIR\forma.exe"
  CreateShortcut "$SMPROGRAMS\Forma\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Forma" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma" "DisplayName" "Forma"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma" "DisplayIcon" "$INSTDIR\forma.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\Forma\Forma.lnk"
  Delete "$SMPROGRAMS\Forma\Uninstall.lnk"
  RMDir "$SMPROGRAMS\Forma"
  Delete "$INSTDIR\forma.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\FONT-LICENSE.txt"
  Delete "$INSTDIR\JETBRAINS-LICENSE.txt"
  Delete "$INSTDIR\INTER-LICENSE.txt"
  Delete "$INSTDIR\PIXELIFY-LICENSE.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Forma"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Forma"
SectionEnd
