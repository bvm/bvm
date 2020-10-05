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
REM Prevent changing the environment for this command
SETLOCAL EnableDelayedExpansion

SET bvm_exec_command=%2
SHIFT
SHIFT

REM Store the remaining args
SET bvm_exec_args=%1
:exec_command_args_loop
SHIFT
IF [%1]==[] GOTO after_exec_command_args_loop
SET bvm_exec_args=%bvm_exec_args% %1
GOTO exec_command_args_loop
:after_exec_command_args_loop

SET bvm_exec_exe_path=

FOR /F "delims=" %%F in ('bvm-bin hidden resolve-command %bvm_exec_command%') do (
  IF !bvm_was_last_exec! == "1" (
    SET bvm_exec_exe_path="%%F"
    GOTO bvmexeccommandfinish
  )
  IF "%%F" == "EXEC" (
    SET bvm_was_last_exec="1"
  ) ELSE (
    %%F
  )
)

REM being here indicates the command above failed
SET bvm_exit_code=1
GOTO end

:bvmexeccommandfinish

%bvm_exec_exe_path% %bvm_exec_args%
SET bvm_exit_code=%ERRORLEVEL%

GOTO end

:bvmexec
REM Prevent changing the environment for this command
SETLOCAL

REM Format: bvm exec [name-selector] [version-selector] <command-name> [...args]
REM Note: Version may include a caret "^1.1.2" so it seems it needs to be referenced directly here
SET bvm_exec_name=%2
SET bvm_exec_command=%4

REM Get if it has the command name
FOR /F "tokens=*" %%F IN ('bvm-bin hidden has-command "%bvm_exec_name%" "%~3" "%bvm_exec_command%"') DO (
  SET bvm_exec_has_command=%%F
)

REM Seems there is no way to get exit code and get output in batch, so just check if it's empty
IF [%bvm_exec_has_command%] == [] (
  SET bvm_exit_code=1
  GOTO end
)

IF "%bvm_exec_has_command%" == "false" SET bvm_exec_command="%bvm_exec_name%"

REM Get the path of the executable
FOR /F "tokens=*" %%F IN ('bvm-bin hidden get-exec-command-path "%bvm_exec_name%" "%~3" "%bvm_exec_command%"') DO (
  SET bvm_exec_exe_path="%%F"
)
IF [%bvm_exec_exe_path%] == [] (
  SET bvm_exit_code=1
  GOTO end
)

REM Run the environment changes
FOR /F "delims=" %%F in ('bvm-bin hidden get-exec-env-changes "%bvm_exec_name%" "%~3"') do (
  %%F
)

REM Remove the already captured args
SHIFT
SHIFT
SHIFT
IF "%bvm_exec_has_command%" == "true" SHIFT

REM Store the remaining args
SET bvm_exec_args=%1
:exec_args_loop
SHIFT
IF [%1]==[] GOTO after_exec_args_loop
SET bvm_exec_args=%bvm_exec_args% %1
GOTO exec_args_loop
:after_exec_args_loop

REM Execute
%bvm_exec_exe_path% %bvm_exec_args%
SET bvm_exit_code=%ERRORLEVEL%

:end
REM Unset any globally set variables (most of them run under SETLOCAL so no need to unset)
SET bvm_had_env_changes=

REM How to clear this before exit?
IF %bvm_exit_code% GTR 0 EXIT /B %bvm_exit_code%
SET bvm_exit_code=
