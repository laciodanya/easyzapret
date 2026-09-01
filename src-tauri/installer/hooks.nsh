; EasyZapret NSIS hooks.
; Before uninstalling we must remove the zapret/WinDivert services and stop
; all the processes that were spawned by the app, otherwise the WinDivert
; driver stays loaded and files stay locked.
;
; Autostart cannot use the Run key: the exe requires administrator rights, and
; Explorer will not launch requireAdministrator binaries at logon. A scheduled
; task with /RL HIGHEST is created instead.

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Removing leftover Run-key autostart..."
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "EasyZapret"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "EasyZapret"
  DetailPrint "Registering EasyZapret logon task..."
  nsExec::ExecToLog 'schtasks /Create /F /RL HIGHEST /SC ONLOGON /DELAY 0000:15 /TN "EasyZapret" /TR "\"$INSTDIR\EasyZapret.exe\""'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Stopping EasyZapret components..."
  nsExec::Exec 'schtasks /Delete /TN "EasyZapret" /F'
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "EasyZapret"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "EasyZapret"
  nsExec::Exec 'taskkill /IM xray.exe /F'
  nsExec::Exec 'taskkill /IM TgWsProxy_windows.exe /F'
  nsExec::Exec 'net stop zapret'
  nsExec::Exec 'sc delete zapret'
  nsExec::Exec 'net stop WinDivert'
  nsExec::Exec 'sc delete WinDivert'
  nsExec::Exec 'net stop WinDivert14'
  nsExec::Exec 'sc delete WinDivert14'
!macroend
