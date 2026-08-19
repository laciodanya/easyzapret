; EasyZapret NSIS hooks.
; Before uninstalling we must remove the zapret/WinDivert services and stop
; all the processes that were spawned by the app, otherwise the WinDivert
; driver stays loaded and files stay locked.

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering EasyZapret login autostart..."
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "EasyZapret" '"$INSTDIR\EasyZapret.exe"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Stopping EasyZapret components..."
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "EasyZapret"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "EasyZapret"
  nsExec::Exec 'taskkill /IM winws.exe /F'
  nsExec::Exec 'taskkill /IM TgWsProxy_windows.exe /F'
  nsExec::Exec 'net stop zapret'
  nsExec::Exec 'sc delete zapret'
  nsExec::Exec 'net stop WinDivert'
  nsExec::Exec 'sc delete WinDivert'
  nsExec::Exec 'net stop WinDivert14'
  nsExec::Exec 'sc delete WinDivert14'
!macroend
