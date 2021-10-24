@echo off

SET bvm_exit_code=0
SET bvm_bin=%~dp0bvm-bin
REM Escape spaces in the path
SET bvm_bin=%bvm_bin: =^ %

IF "%1" == "exec-command" GOTO bvmexeccommand
IF "%1" == "exec" GOTO bvmexec

%bvm_bin% %*
SET bvm_exit_code=%ERRORLEVEL%

IF "%1" == "install" GOTO checkenvchanges
IF "%1" == "uninstall" GOTO checkenvchanges
IF "%1" == "use" GOTO checkenvchanges
GOTO end

:checkenvchanges

REM Check if any changes to the environment are necessary...
SET bvm_had_env_changes=
FOR /F "delims=" %%F in ('%bvm_bin% hidden get-pending-env-changes') do (
  %%F
  SET bvm_had_env_changes=1
)

IF "%bvm_had_env_changes%" == "1" (
  %bvm_bin% hidden clear-pending-env-changes
)

GOTO end

:bvmexeccommand

REM Get the args after the first three by escaping the quotes,
REM surrounding them in quotes, removing the args from the string,
REM then removing the surrounding quotes and unescaping the quotes
SET bvm_exec_args=%*
SET bvm_exec_args=%bvm_exec_args:"=""%
FOR /F "tokens=* USEBACKQ" %%F IN (`%bvm_bin% hidden slice-args 3 true "%bvm_exec_args%"`) DO (
  SET bvm_exec_args=%%F
)
SET bvm_exec_args=%bvm_exec_args:""="%
SET bvm_exec_args=%bvm_exec_args:~1,-1%

REM Enable delayed expansion from here (so that the args above aren't affected)
REM This is necessary for executing the results of resolve-command
SETLOCAL EnableDelayedExpansion

SET bvm_exec_command=%2
SET bvm_exec_exe_path=

REM If these environment variables don't exist, then use the fallback path
if "%USERNAME%" == "" (
  REM The fallback path will already be quoted
  SET bvm_exec_exe_path=%3
  GOTO bvmexeccommandfinish
)

FOR /F "delims=" %%F in ('%bvm_bin% hidden resolve-command %bvm_exec_command%') do (
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
FOR /F "tokens=*" %%F IN ('%bvm_bin% hidden has-command "%bvm_exec_name%" "%~3" "%bvm_exec_command%"') DO (
  SET bvm_exec_has_command=%%F
)

REM Seems there is no way to get exit code and get output in batch, so just check if it's empty
IF [%bvm_exec_has_command%] == [] (
  SET bvm_exit_code=1
  GOTO end
)

IF "%bvm_exec_has_command%" == "false" SET bvm_exec_command="%bvm_exec_name%"

REM Get the path of the executable
FOR /F "tokens=*" %%F IN ('%bvm_bin% hidden get-exec-command-path "%bvm_exec_name%" "%~3" "%bvm_exec_command%"') DO (
  SET bvm_exec_exe_path="%%F"
)
IF [%bvm_exec_exe_path%] == [] (
  SET bvm_exit_code=1
  GOTO end
)

REM Run the environment changes
FOR /F "delims=" %%F in ('%bvm_bin% hidden get-exec-env-changes "%bvm_exec_name%" "%~3"') do (
  %%F
)

REM Store the remaining args
SET bvm_start_arg_index=3
IF "%bvm_exec_has_command%" == "true" (
  SET bvm_start_arg_index=4
)

SET bvm_exec_args=%*
SET bvm_exec_args=%bvm_exec_args:"=""%
FOR /F "tokens=* USEBACKQ" %%F IN (`%bvm_bin% hidden slice-args %bvm_exec_has_command% false "%bvm_exec_args%"`) DO (
  SET bvm_exec_args=%%F
)
SET bvm_exec_args=%bvm_exec_args:~1,-1%
SET bvm_exec_args=%bvm_exec_args:""="%

REM Execute
%bvm_exec_exe_path% %bvm_exec_args%
SET bvm_exit_code=%ERRORLEVEL%
GOTO end

:end
REM Unset any globally set variables (most of them run under SETLOCAL so no need to unset)
SET bvm_had_env_changes=
SET bvm_powershell_path=
SET bvm_bin=
SET bvm_exec_args=

REM How to clear this before exit?
IF %bvm_exit_code% GTR 0 EXIT /B %bvm_exit_code%
SET bvm_exit_code=
