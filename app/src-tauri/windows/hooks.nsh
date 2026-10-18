; Put `bum` on the user's PATH, as the .deb does with /usr/bin/bum, and take it off on uninstall.
;
; PowerShell edits HKCU\Environment\Path rather than NSIS string functions: those cut values at
; 1024 characters, which would truncate a long PATH. The value is read without expanding it and
; written back as REG_EXPAND_SZ, so entries like %USERPROFILE%\bin stay as they are. Setting and
; clearing a throwaway user variable through .NET broadcasts WM_SETTINGCHANGE, so terminals opened
; afterwards see the change without signing out. Both steps are idempotent: the updater runs the
; installer again over an existing install.
;
; Verified on a Windows runner by the "Windows installer puts bum on PATH" step in release.yml.

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = '$INSTDIR'; $$p = (Get-Item 'HKCU:\Environment').GetValue('Path', '', 'DoNotExpandEnvironmentNames'); $$parts = @($$p -split ';' | Where-Object { $$_ }); if ($$parts -notcontains $$d) { New-ItemProperty -Path 'HKCU:\Environment' -Name Path -Value (($$parts + $$d) -join ';') -PropertyType ExpandString -Force | Out-Null; [Environment]::SetEnvironmentVariable('BUMETER_PATH_REFRESH', '1', 'User'); [Environment]::SetEnvironmentVariable('BUMETER_PATH_REFRESH', $$null, 'User') }"`
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = '$INSTDIR'; $$p = (Get-Item 'HKCU:\Environment').GetValue('Path', '', 'DoNotExpandEnvironmentNames'); $$parts = @($$p -split ';' | Where-Object { $$_ }); if ($$parts -contains $$d) { New-ItemProperty -Path 'HKCU:\Environment' -Name Path -Value (($$parts | Where-Object { $$_ -ne $$d }) -join ';') -PropertyType ExpandString -Force | Out-Null; [Environment]::SetEnvironmentVariable('BUMETER_PATH_REFRESH', '1', 'User'); [Environment]::SetEnvironmentVariable('BUMETER_PATH_REFRESH', $$null, 'User') }"`
!macroend
