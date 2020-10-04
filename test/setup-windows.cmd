@echo off

cargo build
copy target\debug\bvm-bin.exe %USERPROFILE%\.bvm\bin\bvm-bin.exe
copy bvm.cmd %USERPROFILE%\.bvm\bin\bvm.cmd
%USERPROFILE%\.bvm\bin\bvm-bin.exe hidden windows-install %USERPROFILE%\.bvm\bin
PATH=%APPDATA%\bvm\shims;%PATH%
