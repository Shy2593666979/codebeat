; Lossless LZMA compression; larger dictionary improves repeated full-CJK font tables.
SetCompressorDictSize 64

!macro NSIS_HOOK_POSTINSTALL
  ; Preserve the current-user application identity across upgrades.
  WriteRegStr HKCU "Software\Classes\AppUserModelId\org.codebeat.app" "DisplayName" "CodeBeat"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\org.codebeat.app" "IconUri" "$INSTDIR\codebeat.exe"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Reuse the native current-user scheduler implementation before deleting binaries.
  nsExec::ExecToLog '"$INSTDIR\codebeat-runner.exe" cleanup-registrations'
  Pop $0
  StrCmp $0 0 cleanup_done
    MessageBox MB_OK|MB_ICONSTOP "CodeBeat could not remove scheduled tasks (exit $0). Uninstall has been stopped; close running tasks and try again."
    Abort
  cleanup_done:
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "codebeat"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\org.codebeat.app"
  ; User data is retained by default; never recursively delete projects or CLI data.
!macroend
