; Extra stappen voor de NSIS-installer van nezty.
;
; Overstap van de oude naam "nzty" naar "nezty": de oude installatie wordt stil verwijderd in
; updatemodus (/UPDATE), zodat instellingen, logboek en back-ups in %APPDATA%\com.nzty.desktop
; blijven staan. Die map hoort bij de app-ID en die is niet veranderd. Daarna worden de oude
; snelkoppelingen en registraties opgeruimd. Stond "starten met Windows" aan, dan gaat dat mee.

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
