; NSIS hooks for the ModSwitch installer.
;
; Uninstalling removes the WebView2 cache, which is disposable (it is regenerated on the next
; run). Nothing is removed during an update ($UpdateMode = 1).
;
; %LOCALAPPDATA%\GtavModManager holds the SQLite database, the vault with the user's mod backups
; and the logs. It is NEVER touched here: keeping it is by design (docs/windows-validation.md §5),
; so a reinstall finds the installations, mods and backups intact. Removing it is a manual
; decision of the user. The app keeps reading it under its old name after the rename.

; ModSwitch was installed as "GTA V Mod Manager" up to v0.5.0. Tauri derives the install folder,
; the uninstall entry and the shortcuts from the product name, so the first ModSwitch installer
; (also when the in-app updater runs it with /UPDATE) installs next to the old app. After the new
; files are in place, this hook removes the old install: its program files, uninstall entry and
; shortcuts. Never its data (above). It does nothing once the old uninstall entry is gone.
!define OLD_PRODUCTNAME "GTA V Mod Manager"
!define OLD_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${OLD_PRODUCTNAME}"

!macro NSIS_HOOK_POSTINSTALL
  Push $R7
  Push $R8
  Push $R9
  ReadRegStr $R8 HKCU "${OLD_UNINSTKEY}" "InstallLocation"
  ReadRegStr $R9 HKCU "${OLD_UNINSTKEY}" "UninstallString"
  ${If} $R9 != ""
    DetailPrint "Removing the previous install (${OLD_PRODUCTNAME})"
    ; Both values are stored with quotes around the path.
    ${WordReplace} "$R8" '"' "" "+" $R8
    ; Was there a desktop shortcut? The old uninstaller removes it; recreate it under the new name.
    StrCpy $R7 0
    ${If} ${FileExists} "$DESKTOP\${OLD_PRODUCTNAME}.lnk"
      StrCpy $R7 1
    ${EndIf}
    ${If} $R8 != ""
    ${AndIf} $R8 != "$INSTDIR"
    ${AndIf} ${FileExists} "$R8\uninstall.exe"
      ; Silent and without /UPDATE, so it also removes its shortcuts and its uninstall entry.
      ; _?= runs it in place and makes ExecWait wait for it.
      ExecWait '"$R8\uninstall.exe" /S _?=$R8'
      Delete "$R8\uninstall.exe"
      RMDir "$R8"
    ${EndIf}
    ; Whatever the old uninstaller did not get to (missing uninstaller, same folder).
    DeleteRegKey HKCU "${OLD_UNINSTKEY}"
    DeleteRegKey HKCU "${MANUKEY}\${OLD_PRODUCTNAME}"
    Delete "$SMPROGRAMS\${OLD_PRODUCTNAME}.lnk"
    Delete "$DESKTOP\${OLD_PRODUCTNAME}.lnk"
    ; An update (/UPDATE) creates no shortcuts: without these the app would vanish from Start.
    ${IfNot} ${FileExists} "$SMPROGRAMS\${PRODUCTNAME}.lnk"
      CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"
    ${EndIf}
    ${If} $R7 = 1
    ${AndIfNot} ${FileExists} "$DESKTOP\${PRODUCTNAME}.lnk"
      CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
      !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
    ${EndIf}
  ${EndIf}
  Pop $R9
  Pop $R8
  Pop $R7
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    RMDir /r "$LOCALAPPDATA\${BUNDLEID}\EBWebView"
  ${EndIf}
!macroend
