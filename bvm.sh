#!/bin/sh
bvm="bvm-bin"

if [ "$1" = "get-new-path" ]
then
  $bvm_bin hidden-shell get-new-path "$PATH"
  exit $?
fi

new_path=$(bvm-bin hidden-shell get-new-path "$PATH")
if [ "$PATH" != "$new_path" ]
then
  $bvm_bin hidden-shell clear-pending-changes
fi

if [ "$1" = "exec" ]
then
  # Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
  bvm_exec_name=$2
  bvm_exec_version=$3
  bvm_exec_command=$4
  executable_path=$($bvm_bin hidden-shell get-exec-command-path "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { exit $?; }

  export PATH=$($bvm_bin hidden-shell get-exec-env-path "$bvm_exec_name" "$bvm_exec_version" "%PATH%") || { exit $?; }

  shift 4
  $executable_path "$@"
  exit $?
fi

$bvm_bin "$@"

if [ "$1" = "install" ] || [ "$1" = "uninstall" ] || [ "$1" = "use" ]
then
  new_path=$(bvm-bin hidden-shell get-new-path "$PATH")
  if [ "$PATH" != "$new_path" ]
  then
    echo 'The path has changed based. To update it run:'
    echo 'export PATH=$(bvm get-new-path)'
  fi
fi
