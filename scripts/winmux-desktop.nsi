;-------------------------------------------------------------
; WinMux Desktop installer (NSIS)
; Per-user, no admin needed
;-------------------------------------------------------------

!ifndef VERSION
  !define VERSION "0.1.13"
!endif

!ifndef BUILD_DIR
  !define BUILD_DIR ".\build\desktop"
!endif

!ifndef DIST_DIR
  !define DIST_DIR ".\dist"
!endif

!define APP_NAME "WinMux Desktop"
!define APP_PUBLISHER "Denis Romanyuk"
!define APP_DIR "WinMux"
!define APP_EXE "winmux-desktop.exe"

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

Name "${APP_NAME} ${VERSION}"
OutFile "${DIST_DIR}\winmux-desktop-setup-v${VERSION}.exe"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\${APP_DIR}"
; Деінтенсивно: завжди default → LOCALAPPDATA, навіть якщо в реєстрі стара локація.
; (раніше InstallDirRegKey тягнув недоступний шлях типу redirected E:\)

ShowInstDetails show
ShowUninstDetails show
SetCompressor /SOLID lzma
SetCompressorDictSize 32

!define MUI_ABORTWARNING
!define MUI_ICON "${BUILD_DIR}\..\..\code\winmux-desktop\src-tauri\icons\icon.ico"
!define MUI_UNICON "${BUILD_DIR}\..\..\code\winmux-desktop\src-tauri\icons\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Launch WinMux Desktop"
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\README.txt"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Ukrainian"

Section "Install"
  SetOutPath "$INSTDIR"
  File "${BUILD_DIR}\winmux.exe"
  File "${BUILD_DIR}\winmux-desktop.exe"
  File "${BUILD_DIR}\WebView2Loader.dll"
  File "${BUILD_DIR}\winmux.toml"
  File "${BUILD_DIR}\README.txt"

  ; --- WebView2 Runtime check ---
  ; Tauri requires Microsoft Edge WebView2 Runtime (separate from our bundled
  ; WebView2Loader.dll, which is just the loader). Win11 + most updated Win10
  ; have it. Older Win10 / fresh Server 2019 — no. Auto-install via Evergreen
  ; bootstrapper if missing (~1.5 MB download, no admin prompt).
  ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 == ""
    ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
  ${If} $0 == ""
    ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\ClientState\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
  ${If} $0 == ""
    DetailPrint "WebView2 Runtime not found — downloading via PowerShell (handles HTTPS redirects)..."
    InitPluginsDir
    nsExec::ExecToLog 'powershell -NoProfile -ExecutionPolicy Bypass -Command "try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri \"https://go.microsoft.com/fwlink/p/?LinkId=2124703\" -OutFile \"$PLUGINSDIR\\MicrosoftEdgeWebview2Setup.exe\" -UseBasicParsing -TimeoutSec 60; exit 0 } catch { exit 1 }"'
    Pop $0
    ${If} $0 == "0"
      DetailPrint "Installing WebView2 Runtime (silent, no admin prompt)..."
      ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install' $1
      DetailPrint "WebView2 installer exit code: $1"
    ${Else}
      DetailPrint "WebView2 download failed (PS exit $0). Install manually from https://go.microsoft.com/fwlink/p/?LinkId=2124703 if Desktop fails to launch."
      MessageBox MB_OK|MB_ICONEXCLAMATION "WinMux installed, but Microsoft Edge WebView2 Runtime is missing and could not be auto-downloaded. Open https://go.microsoft.com/fwlink/p/?LinkId=2124703 in a browser, install it, then launch WinMux Desktop."
    ${EndIf}
  ${Else}
    DetailPrint "WebView2 Runtime detected: $0"
  ${EndIf}

  SetOutPath "$INSTDIR\qemu"
  File /r "${BUILD_DIR}\qemu\*.*"

  SetOutPath "$INSTDIR\rootfs"
  File /r "${BUILD_DIR}\rootfs\*.*"

  SetOutPath "$INSTDIR\logs"
  File /nonfatal "${BUILD_DIR}\logs\*.*"

  WriteRegStr HKCU "Software\${APP_DIR}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\${APP_DIR}" "Version" "${VERSION}"

  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "NoRepair" 1

  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop" \
      "EstimatedSize" "$0"

  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\WinMux Desktop.lnk" \
      "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\WinMux PowerShell.lnk" \
      "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" \
      "-NoExit -Command Set-Location '$INSTDIR'" \
      "$INSTDIR\winmux.exe" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  CreateShortcut "$DESKTOP\WinMux.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0

  ; --- Windows Explorer context menu: "Open in WinMux" ---
  ; Right-click on a folder → run WinMux with that folder
  WriteRegStr HKCU "Software\Classes\Directory\shell\WinMux" "" "Open in WinMux"
  WriteRegStr HKCU "Software\Classes\Directory\shell\WinMux" "Icon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "Software\Classes\Directory\shell\WinMux\command" "" '"$INSTDIR\${APP_EXE}" "%1"'
  ; Right-click on empty space inside a folder
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\WinMux" "" "Open in WinMux"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\WinMux" "Icon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "Software\Classes\Directory\Background\shell\WinMux\command" "" '"$INSTDIR\${APP_EXE}" "%V"'
  ; Right-click on a drive
  WriteRegStr HKCU "Software\Classes\Drive\shell\WinMux" "" "Open in WinMux"
  WriteRegStr HKCU "Software\Classes\Drive\shell\WinMux" "Icon" "$INSTDIR\${APP_EXE}"
  WriteRegStr HKCU "Software\Classes\Drive\shell\WinMux\command" "" '"$INSTDIR\${APP_EXE}" "%1"'

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  DetailPrint "Installed to $INSTDIR"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\winmux.exe"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\winmux.toml"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$INSTDIR\user.qcow2"
  Delete "$INSTDIR\boot.log"

  RMDir /r "$INSTDIR\qemu"
  RMDir /r "$INSTDIR\rootfs"
  RMDir /r "$INSTDIR\logs"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\WinMux Desktop.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\WinMux PowerShell.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk"
  Delete "$DESKTOP\WinMux.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  DeleteRegKey HKCU "Software\${APP_DIR}"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-Desktop"
  ; Context menu cleanup
  DeleteRegKey HKCU "Software\Classes\Directory\shell\WinMux"
  DeleteRegKey HKCU "Software\Classes\Directory\Background\shell\WinMux"
  DeleteRegKey HKCU "Software\Classes\Drive\shell\WinMux"
SectionEnd
