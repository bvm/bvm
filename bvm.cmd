@echo off

bvm-bin %*

if "%1" == "install" GOTO checkpathchanges
if "%1" == "uninstall" GOTO checkpathchanges
if "%1" == "use" GOTO checkpathchanges
GOTO end

:checkpathchanges

REM Check if any changes on the path are necessary...
FOR /F "tokens=*" %%F IN ('bvm-bin hidden-shell get-new-path "%PATH%"') DO (
  SET new_path=%%F
)

if "%PATH%" == "%new_path%" GOTO end
SET PATH=%new_path%
bvm-bin hidden-shell clear-pending-changes

:end