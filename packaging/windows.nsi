Unicode true
!include "MUI2.nsh"
!include "x64.nsh"

Name "JSON Lens"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\JSON Lens"
InstallDirRegKey HKCU "Software\JSON Lens" "InstallDir"
RequestExecutionLevel user
ManifestDPIAware true

!define MUI_ABORTWARNING
!define MUI_ICON "${STAGE}\json-lens.ico"
!define MUI_UNICON "${STAGE}\json-lens.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\json-lens.exe"
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
    MessageBox MB_ICONSTOP "JSON Lens requires 64-bit Windows."
    Abort
  ${EndIf}
  SetShellVarContext current
FunctionEnd

Section "JSON Lens"
  SetOutPath "$INSTDIR"
  File "${STAGE}\json-lens.exe"
  File "${STAGE}\LICENSE"
  File "${STAGE}\FONT-LICENSE.txt"
  File "${STAGE}\JETBRAINS-LICENSE.txt"
  File "${STAGE}\INTER-LICENSE.txt"
  File "${STAGE}\PIXELIFY-LICENSE.txt"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\JSON Lens"
  CreateShortcut "$SMPROGRAMS\JSON Lens\JSON Lens.lnk" "$INSTDIR\json-lens.exe"
  CreateShortcut "$SMPROGRAMS\JSON Lens\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\JSON Lens" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens" "DisplayName" "JSON Lens"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens" "DisplayIcon" "$INSTDIR\json-lens.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Delete "$SMPROGRAMS\JSON Lens\JSON Lens.lnk"
  Delete "$SMPROGRAMS\JSON Lens\Uninstall.lnk"
  RMDir "$SMPROGRAMS\JSON Lens"
  Delete "$INSTDIR\json-lens.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\FONT-LICENSE.txt"
  Delete "$INSTDIR\JETBRAINS-LICENSE.txt"
  Delete "$INSTDIR\INTER-LICENSE.txt"
  Delete "$INSTDIR\PIXELIFY-LICENSE.txt"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\JSON Lens"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\JSON Lens"
SectionEnd
