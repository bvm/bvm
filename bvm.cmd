@echo off

if "%1" == "exec" GOTO bvmexec

bvm-bin %*
set bvm_exit_code=%ERRORLEVEL%

if "%1" == "install" GOTO checkpathchanges
if "%1" == "uninstall" GOTO checkpathchanges
if "%1" == "use" GOTO checkpathchanges
GOTO end

:checkpathchanges

REM Check if any changes on the path are necessary...
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell get-new-path "%PATH%"') DO (
  SET bvm_new_path=%%F
)

if "%PATH%" == "%bvm_new_path%" GOTO end
SET PATH=%bvm_new_path%
bvm-bin hidden-shell clear-pending-changes

GOTO end

:bvmexec
REM Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
set bvm_exec_name=%2
set bvm_exec_version=%3
set bvm_exec_command=%4
set bvm_exec_old_env_path=%PATH%

REM Get if it has the command name
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell has-command "%bvm_exec_name%" "%bvm_exec_version%" "%bvm_exec_command%"') DO (
  SET bvm_exec_has_command=%%F
)

REM Seems there is no way to get exit code and get output in batch, so just check if it's empty
IF [%bvm_exec_has_command%] == [] GOTO end

if "%bvm_exec_has_command%" == "false" SET bvm_exec_command="%bvm_exec_name%"

REM Remove the already captured args
shift
shift
shift
if "%bvm_exec_has_command%" == "true" shift

REM Now store the remaining args
set bvm_exec_args=%1
:loop
shift
if [%1]==[] goto afterloop
set bvm_exec_args=%bvm_exec_args% %1
goto loop
:afterloop

REM Get the path of the executable
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell get-exec-command-path "%bvm_exec_name%" "%bvm_exec_version%" "%bvm_exec_command%"') DO (
  SET bvm_exec_exe_path=%%F
)
IF [%bvm_exec_exe_path%] == [] GOTO end

REM Get the new env path
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell get-exec-env-path "%bvm_exec_name%" "%bvm_exec_version%" "%PATH%"') DO (
  SET bvm_exec_env_path=%%F
)

set bvm_exec_old_path=%PATH%
set PATH=%bvm_exec_env_path%
"%bvm_exec_exe_path%" %bvm_exec_args%
set bvm_exit_code=%ERRORLEVEL%
set PATH=%bvm_exec_old_path%

:end
set bvm_new_path=
set bvm_exec_name=
set bvm_exec_version=
set bvm_exec_command=
set bvm_exec_old_path=
set bvm_exec_has_command=
set bvm_exec_exe_path=
set bvm_exec_env_path=

REM How to clear this before exit?
if %bvm_exit_code% GEQ 1 EXIT /B %bvm_exit_code%
set bvm_exit_code=
