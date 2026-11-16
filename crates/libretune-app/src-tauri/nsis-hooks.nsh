; NSIS installer hooks for LibreTune (wired in via
; bundle.windows.nsis.installerHooks in tauri.conf.json). Macros compiled
; here run in every install mode, including the silent /UPDATE installs the
; in-app updater performs.

!macro NSIS_HOOK_POSTINSTALL
  ; productName was renamed libretune-app -> LibreTune in v0.1.1. Everything
  ; the NSIS installer keys off (registry install-dir + uninstall keys,
  ; shortcut names, default install dir) changed with it, so an in-app update
  ; from a pre-rename install leaves the old entry behind: a duplicate
  ; Add/Remove-Programs item, stale Start-Menu/Desktop shortcuts pointing at
  ; the abandoned directory, and the directory itself. The updater also runs
  ; with /UPDATE, which skips the template's normal shortcut creation, so the
  ; new shortcut has to be (re)created here. Every step below is a guarded
  ; no-op when no pre-rename install exists (fresh installs, future updates).

  ; Ensure the Start-Menu shortcut exists for THIS install (the /UPDATE path
  ; never creates it; on fresh installs this just overwrites in place).
  CreateShortcut "$SMPROGRAMS\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  !insertmacro SetLnkAppUserModelId "$SMPROGRAMS\${PRODUCTNAME}.lnk"

  ; Retire the legacy Start-Menu shortcut.
  ${If} ${FileExists} "$SMPROGRAMS\libretune-app.lnk"
    !insertmacro UnpinShortcut "$SMPROGRAMS\libretune-app.lnk"
    Delete "$SMPROGRAMS\libretune-app.lnk"
  ${EndIf}

  ; Replace the legacy Desktop shortcut, but only if the user had one.
  ${If} ${FileExists} "$DESKTOP\libretune-app.lnk"
    CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
    !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
    !insertmacro UnpinShortcut "$DESKTOP\libretune-app.lnk"
    Delete "$DESKTOP\libretune-app.lnk"
  ${EndIf}

  ; Remove the legacy per-user install (installMode is currentUser, so its
  ; state lives under HKCU / $LOCALAPPDATA). Prefer its own uninstaller so
  ; the exact registry set is retired, then sweep whatever is left. Never
  ; touch it in the unlikely case it is the current $INSTDIR.
  ${If} "$LOCALAPPDATA\libretune-app" != "$INSTDIR"
    ${If} ${FileExists} "$LOCALAPPDATA\libretune-app\uninstall.exe"
      ExecWait '"$LOCALAPPDATA\libretune-app\uninstall.exe" /S _?="$LOCALAPPDATA\libretune-app"' $0
    ${EndIf}
    RMDir /r "$LOCALAPPDATA\libretune-app"
    ; The silent legacy uninstaller skips these (they sit behind its
    ; "delete app data" checkbox path).
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\libretune-app"
    DeleteRegKey HKCU "Software\libretune\libretune-app"
  ${EndIf}
!macroend
