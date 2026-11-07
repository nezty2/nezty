; The per-machine build only registers elevated tasks for this protected location.
!macro NSIS_HOOK_PREINSTALL
  ${If} $INSTDIR != "$PROGRAMFILES64\nezty"
    MessageBox MB_ICONSTOP "Installeer Nezty in $PROGRAMFILES64\nezty." /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  ; Preserve the autostart choice, but retire tasks targeting per-user installations.
  nsExec::ExecToStack '"$SYSDIR\schtasks.exe" /Query /TN "nezty autostart"'
  Pop $R9
  Pop $R8
  ${If} $R9 == 0
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "nezty" '"$INSTDIR\nezty.exe" --minimized'
    nsExec::ExecToLog '"$SYSDIR\schtasks.exe" /Delete /TN "nezty autostart" /F'
    Pop $R9
    ${If} $R9 != 0
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
  nsExec::ExecToStack '"$SYSDIR\schtasks.exe" /Query /TN "nezty licentiecontrole"'
  Pop $R9
  Pop $R8
  ${If} $R9 == 0
    nsExec::ExecToLog '"$SYSDIR\schtasks.exe" /Delete /TN "nezty licentiecontrole" /F'
    Pop $R9
    ${If} $R9 != 0
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Retire only the known per-user registrations. An old uninstaller in a
  ; writable directory must never run from this elevated installation.
  SetShellVarContext current
  ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\nezty" "InstallLocation"
  ${If} $R7 != ""
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\nezty"
    Delete "$SMPROGRAMS\nezty.lnk"
    Delete "$DESKTOP\nezty.lnk"
  ${EndIf}
  ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\nzty" "InstallLocation"
  ${If} $R7 != ""
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\nzty"
    Delete "$SMPROGRAMS\nzty.lnk"
    Delete "$DESKTOP\nzty.lnk"
  ${EndIf}
  SetShellVarContext all
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ; The per-machine uninstaller already runs elevated. Fail closed if launch
    ; fails, recovery is incomplete, or a different account still needs Nezty.
    ClearErrors
    StrCpy $R9 -1
    ExecWait '"$INSTDIR\nezty.exe" --restore-all' $R9
    ${If} ${Errors}
    ${OrIf} $R9 != 0
      MessageBox MB_ICONSTOP "Herstel is niet voltooid. Nezty en de herstelgegevens blijven beschikbaar. Bekijk %TEMP%\nezty-restore-all.json en probeer het opnieuw." /SD IDOK
      SetErrorLevel 1
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    RMDir /r "$APPDATA\com.nzty.desktop"
    RMDir /r "$LOCALAPPDATA\com.nzty.desktop"
    Delete "$DESKTOP\nezty.lnk"
    SetShellVarContext all
  ${EndIf}
!macroend
