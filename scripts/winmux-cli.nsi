;-------------------------------------------------------------
; WinMux CLI installer (NSIS)
; Per-user, no admin needed
;-------------------------------------------------------------

!ifndef VERSION
  !define VERSION "0.1.8"
!endif

!ifndef BUILD_DIR
  !define BUILD_DIR ".\build\cli"
!endif

!ifndef DIST_DIR
  !define DIST_DIR ".\dist"
!endif

!define APP_NAME "WinMux CLI"
!define APP_PUBLISHER "Denis Romanyuk"
!define APP_DIR "WinMux"
!define APP_EXE "winmux.exe"

;-- Modern UI --
!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "${APP_NAME} ${VERSION}"
OutFile "${DIST_DIR}\winmux-cli-setup-v${VERSION}.exe"
RequestExecutionLevel user                 ; per-user, no UAC
InstallDir "$LOCALAPPDATA\${APP_DIR}"

ShowInstDetails show
ShowUninstDetails show
SetCompressor /SOLID lzma
SetCompressorDictSize 32

;-- UI pages --
!define MUI_ABORTWARNING
!define MUI_ICON "${BUILD_DIR}\..\..\code\winmux-desktop\src-tauri\icons\icon.ico"
!define MUI_UNICON "${BUILD_DIR}\..\..\code\winmux-desktop\src-tauri\icons\icon.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\README.txt"
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Ukrainian"

;-- Install section --
Section "Install"
  SetOutPath "$INSTDIR"

  ; Бінарник + конфіг + readme
  File "${BUILD_DIR}\winmux.exe"
  File "${BUILD_DIR}\winmux.toml"
  File "${BUILD_DIR}\winmux.ps1"
  File "${BUILD_DIR}\README.txt"

  ; QEMU bundle
  SetOutPath "$INSTDIR\qemu"
  File /r "${BUILD_DIR}\qemu\*.*"

  ; Rootfs
  SetOutPath "$INSTDIR\rootfs"
  File /r "${BUILD_DIR}\rootfs\*.*"

  ; Logs folder placeholder
  SetOutPath "$INSTDIR\logs"
  File /nonfatal "${BUILD_DIR}\logs\*.*"

  ; Реєстр (для Add/Remove Programs)
  WriteRegStr HKCU "Software\${APP_DIR}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\${APP_DIR}" "Version" "${VERSION}"

  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "NoRepair" 1

  ; Розмір (для Add/Remove)
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI" \
      "EstimatedSize" "$0"

  ; PATH integration (optional, у per-user PATH)
  ; Це закоментовано за замовчуванням бо потребує оновлення PATH у поточних shell-ах.
  ; EnVar::SetHKCU
  ; EnVar::AddValue "PATH" "$INSTDIR"

  ; Start Menu shortcuts
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\WinMux PowerShell.lnk" \
      "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" \
      "-NoExit -Command Set-Location '$INSTDIR'" \
      "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\Configuration.lnk" \
      "notepad.exe" "$INSTDIR\winmux.toml"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk" \
      "$INSTDIR\Uninstall.exe"

  ; Uninstaller
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  DetailPrint "Installed to $INSTDIR"
SectionEnd

;-- Uninstall --
Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\winmux.toml"
  Delete "$INSTDIR\winmux.ps1"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\Uninstall.exe"

  RMDir /r "$INSTDIR\qemu"
  RMDir /r "$INSTDIR\rootfs"
  RMDir /r "$INSTDIR\logs"
  ; user.qcow2 (overlay), boot.log, etc — створюються в runtime
  Delete "$INSTDIR\user.qcow2"
  Delete "$INSTDIR\boot.log"

  ; Spróбуємо видалити директорію якщо порожня
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\WinMux PowerShell.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Configuration.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Uninstall.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  DeleteRegKey HKCU "Software\${APP_DIR}"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_DIR}-CLI"
SectionEnd
