@echo off

SET bvm_exit_code=0

IF "%1" == "exec-command" GOTO bvmexeccommand
IF "%1" == "exec" GOTO bvmexec

bvm-bin %*
SET bvm_exit_code=%ERRORLEVEL%

IF "%1" == "install" GOTO checkenvchanges
IF "%1" == "uninstall" GOTO checkenvchanges
IF "%1" == "use" GOTO checkenvchanges
GOTO end

:checkenvchanges

REM Check if any changes to the environment are necessary...
SET bvm_had_env_changes=
FOR /F "delims=" %%F in ('bvm-bin hidden get-pending-env-changes') do (
  %%F
  SET bvm_had_env_changes=1
)

IF "%bvm_had_env_changes%" == "1" (
  bvm-bin hidden clear-pending-env-changes
)

GOTO end

:bvmexeccommand
:bvmexec
SETLOCAL

REM Escape escaped double quotes for powershell
set bvm_exec_args=%*
set bvm_exec_args=%bvm_exec_args:\"=`"%
REM Escape double quotes for the string below
set bvm_exec_args=%bvm_exec_args:"=\"%

REM Handle powershell not existing on the path
set bvm_powershell_path=C:\Windows\System32\WINDOWSPOWERSHELL\v1.0\powershell.exe
where /q Powershell >nul 2>nul
IF "%ERRORLEVEL%" == "0" (
  set bvm_powershell_path=Powershell
)

REM Run these in powershell so it handles arguments like `lib=""`
%bvm_powershell_path% -ExecutionPolicy Bypass -Command "& %~dp0bvm.ps1 %bvm_exec_args%; exit $lastexitcode"
SET bvm_exit_code=%ERRORLEVEL%
GOTO end

:end
REM Unset any globally set variables (most of them run under SETLOCAL so no need to unset)
SET bvm_had_env_changes=
SET bvm_powershell_path=

REM How to clear this before exit?
IF %bvm_exit_code% GTR 0 EXIT /B %bvm_exit_code%
SET bvm_exit_code=
