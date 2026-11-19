; NSIS hooks for the GTA V Mod Manager installer.
;
; Uninstalling removes the WebView2 cache, which is disposable (it is regenerated on the next
; run). Nothing is removed during an update ($UpdateMode = 1).
;
; %LOCALAPPDATA%\GtavModManager holds the SQLite database, the vault with the user's mod backups
; and the logs. It is NEVER touched here: keeping it is by design (docs/windows-validation.md §5),
; so a reinstall finds the installations, mods and backups intact. Removing it is a manual
; decision of the user.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    RMDir /r "$LOCALAPPDATA\${BUNDLEID}\EBWebView"
  ${EndIf}
!macroend
