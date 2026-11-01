; Extra stappen voor de NSIS-installer van nezty.
;
; 1. Overstap van de oude naam "nzty" naar "nezty": de oude installatie wordt stil verwijderd in
;    updatemodus (/UPDATE), zodat instellingen, logboek en back-ups in %APPDATA%\com.nzty.desktop
;    blijven staan. Die map hoort bij de app-ID en die is niet veranderd. Daarna worden de oude
;    snelkoppelingen en registraties opgeruimd. Stond "starten met Windows" aan, dan gaat dat mee.
; 2. Verwijderen = alles terug. Vóór de uninstaller iets weghaalt, draait hij "nezty.exe --restore-all":
;    elke gelogde wijziging gaat terug naar hoe het stond, de geplande taken en het bewaarde token gaan weg.
;    Daarna gaan ook de mappen in AppData weg. Bij een update (/UPDATE) gebeurt dit allemaal niet:
;    dan neemt de nieuwe versie alles over.

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R7 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\nzty" "InstallLocation"
  ${If} $R7 != ""
    ; Aanhalingstekens om het pad weghalen.
    StrCpy $R9 $R7 1
    ${If} $R9 == '"'
      StrCpy $R7 $R7 "" 1
      StrCpy $R7 $R7 -1
    ${EndIf}
    ReadRegStr $R8 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "nzty"
    ${If} $R7 != $INSTDIR
      ${If} ${FileExists} "$R7\uninstall.exe"
        ExecWait '"$R7\uninstall.exe" /S /UPDATE _?=$R7'
      ${EndIf}
      ; Lukte het verwijderen niet (bijvoorbeeld omdat Windows het oude programma blokkeert),
      ; dan alleen de bekende programmabestanden weghalen. Nooit iets anders in die map.
      Delete "$R7\nzty.exe"
      Delete "$R7\WebView2Loader.dll"
      Delete "$R7\resources\PresentMon.exe"
      Delete "$R7\resources\PresentMon-LICENSE.txt"
      Delete "$R7\uninstall.exe"
      RMDir "$R7\resources"
      RMDir "$R7"
    ${EndIf}
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\nzty"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "nzty"
    Delete "$SMPROGRAMS\nzty.lnk"
    Delete "$DESKTOP\nzty.lnk"
    ${If} $R8 != ""
      WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "nezty" '"$INSTDIR\nezty.exe" --minimized'
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ${If} ${FileExists} "$INSTDIR\nezty.exe"
      ; nezty draait met administratorrechten, dus dit vraagt één keer om toestemming (UAC).
      ; --restore-all sluit eerst een draaiende nezty, zet dan alles terug en ruimt taken en token op.
      ; De uitkomst staat in %TEMP%\nezty-restore-all.json.
      ExecShellWait "runas" "$INSTDIR\nezty.exe" "--restore-all"
    ${EndIf}
    ; Voor het geval nezty.exe zelf niet kon draaien: geplande taken en autostart alsnog weg.
    nsExec::ExecToLog 'schtasks /Delete /TN "nezty autostart" /F'
    Pop $R9
    nsExec::ExecToLog 'schtasks /Delete /TN "nezty licentiecontrole" /F'
    Pop $R9
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "nezty"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; Instellingen, logboek, covers, back-ups en logbestanden: na verwijderen blijft er niets achter.
    SetShellVarContext current
    RMDir /r "$APPDATA\com.nzty.desktop"
    RMDir /r "$LOCALAPPDATA\com.nzty.desktop"
    Delete "$DESKTOP\nezty.lnk"
  ${EndIf}
!macroend
