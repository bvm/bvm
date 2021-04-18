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

REM Escape all double quotes
set bvm_exec_args=%*
set bvm_exec_args=%bvm_exec_args:"=\"%

REM Run these in powershell so it handles arguments like `lib=""`
Powershell -ExecutionPolicy Bypass -Command "& bvm.ps1 %bvm_exec_args%"
SET bvm_exit_code=%ERRORLEVEL%
GOTO end

:end
REM Unset any globally set variables (most of them run under SETLOCAL so no need to unset)
SET bvm_had_env_changes=

REM How to clear this before exit?
IF %bvm_exit_code% GTR 0 EXIT /B %bvm_exit_code%
SET bvm_exit_code=
