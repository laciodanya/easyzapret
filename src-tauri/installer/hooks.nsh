; EasyZapret NSIS hooks.
; Before uninstalling we must remove the zapret/WinDivert services and stop
; all the processes that were spawned by the app, otherwise the WinDivert
; driver stays loaded and files stay locked.
;
; Autostart cannot use the Run key: the exe requires administrator rights, and
; Explorer will not launch requireAdministrator binaries at logon. A scheduled
; task is created only if the user enables launch-at-login in Settings — never
; from the installer (Defender flags silent schtasks as Persistence.A!ml).

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Removing leftover autostart entries..."
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "EasyZapret"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "EasyZapret"
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
