@echo off

SET bvm_exit_code=0

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
for /F "delims=" %%F in ('bvm-bin hidden-shell get-pending-env-changes') do (
  %%F
  SET bvm_had_env_changes=1
)

IF "%bvm_had_env_changes%" == "1" (
  bvm-bin hidden-shell clear-pending-env-changes
)

GOTO end

:bvmexec
REM Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
SET bvm_exec_name=%2
SET bvm_exec_version=%3
SET bvm_exec_command=%4

REM Get if it has the command name
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell has-command "%bvm_exec_name%" "%bvm_exec_version%" "%bvm_exec_command%"') DO (
  SET bvm_exec_has_command=%%F
)

REM Seems there is no way to get exit code and get output in batch, so just check if it's empty
IF [%bvm_exec_has_command%] == [] (
  SET bvm_exit_code=1
  GOTO end
)

IF "%bvm_exec_has_command%" == "false" SET bvm_exec_command="%bvm_exec_name%"

REM Remove the already captured args
SHIFT
SHIFT
SHIFT
IF "%bvm_exec_has_command%" == "true" SHIFT

REM Now store the remaining args
SET bvm_exec_args=%1
:loop
SHIFT
IF [%1]==[] GOTO afterloop
SET bvm_exec_args=%bvm_exec_args% %1
GOTO loop
:afterloop

REM Get the path of the executable
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell get-exec-command-path "%bvm_exec_name%" "%bvm_exec_version%" "%bvm_exec_command%"') DO (
  SET bvm_exec_exe_path="%%F"
)
IF [%bvm_exec_exe_path%] == [] (
  SET bvm_exit_code=1
  GOTO end
)

REM Run the environment changes
FOR /F "delims=" %%F in ('bvm-bin hidden-shell get-exec-env-changes "%bvm_exec_name%" "%bvm_exec_version%"') do (
  %%F
)

%bvm_exec_exe_path% %bvm_exec_args%
SET bvm_exit_code=%ERRORLEVEL%

REM Run the post environment changes
FOR /F "delims=" %%F in ('bvm-bin hidden-shell get-post-exec-env-changes "%bvm_exec_name%" "%bvm_exec_version%"') do (
  %%F
)

:end
SET bvm_exec_name=
SET bvm_exec_version=
SET bvm_exec_command=
SET bvm_exec_has_command=
SET bvm_exec_exe_path=
SET bvm_had_env_changes=

REM How to clear this before exit?
IF %bvm_exit_code% GTR 0 EXIT /B %bvm_exit_code%
SET bvm_exit_code=