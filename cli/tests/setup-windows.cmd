@echo off

cargo build
copy target\debug\bvm-bin.exe %USERPROFILE%\.bvm\bin\bvm-bin.exe
copy cli\bvm.cmd %USERPROFILE%\.bvm\bin\bvm.cmd
copy cli\bvm.ps1 %USERPROFILE%\.bvm\bin\bvm.ps1
%USERPROFILE%\.bvm\bin\bvm-bin.exe hidden windows-install
PATH=%APPDATA%\bvm\shims;%PATH%
