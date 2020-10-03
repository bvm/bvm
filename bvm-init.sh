#!/bin/sh

bvm_handle_env_messages()
{
  # * ADD\n<key>\n<value>
  # * REMOVE\n<key>
  local message_step
  message_step=0
  local bvm_var_name
  local IFS
  IFS='
'
  for line in $1
  do
    if [ "$message_step" = "0" ]
    then
      if [ "$line" = "ADD" ]
      then
        message_step=1
      elif [ "$line" = "REMOVE" ]
      then
        message_step=10
      else
        echo "Internal error. Unexpected output: $line"
        return 1
      fi
    elif [ "$message_step" = "1" ] # adding, get var name
    then
      bvm_var_name="$line"
      message_step=2
    elif [ "$message_step" = "2" ] # adding, get var value
    then
      printf -v "$bvm_var_name" "$line"
      export $bvm_var_name

      message_step=0
    elif [ "$message_step" = "10" ] # removing, get var value
    then
      printf -v "$line" ""
      export $line
      unset "$line"
      message_step=0
    else
      echo "Internal error. Unexpected output: $line"
      return 1
    fi
  done
}

bvm()
{
  local bvm_bin
  bvm_bin="$BVM_INSTALL_DIR/bin/bvm-bin"

  if [ "$1" = "exec" ]
  then
    # Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
    local bvm_exec_name
    local bvm_exec_version
    local bvm_exec_command
    local bvm_has_command
    bvm_exec_name=$2
    bvm_exec_version=$3
    bvm_exec_command=$4
    bvm_has_command=$($bvm_bin hidden-shell has-command "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { return $?; }

    if [ "$bvm_has_command" = "false" ]
    then
      bvm_exec_command=$bvm_exec_name
    fi

    local bvm_executable_path
    bvm_executable_path=$($bvm_bin hidden-shell get-exec-command-path "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { return $?; }

    if [ "$bvm_has_command" = "false" ]
    then
      shift 3
    else
      shift 4
    fi
    local bvm_exec_args
    bvm_exec_args="$@"

    # use a sub shell to prevent exporting variables
    (
      bvm_handle_env_messages "$($bvm_bin hidden-shell get-exec-env-changes "$bvm_exec_name" "$bvm_exec_version")" || { return $?; }

      $bvm_executable_path $bvm_exec_args
    )
    return $?
  fi

  $bvm_bin "$@"

  if [ "$1" = "install" ] || [ "$1" = "uninstall" ] || [ "$1" = "use" ]
  then
    local pending_changes
    pending_changes=$($bvm_bin hidden-shell get-pending-env-changes)
    bvm_handle_env_messages "$pending_changes" || { return $?; }
  fi
}

if [ -z "$BVM_INSTALL_DIR" ]
then
  echo "You must specify a BVM_INSTALL_DIR environment variable (ex. \`export BVM_INSTALL_DIR=\"$HOME/.bvm\"\`)."
else
  # use the bin directly since we haven't set the path yet
  bvm_binary_paths=$($BVM_INSTALL_DIR/bin/bvm-bin hidden-shell get-paths)

  bvm_handle_env_messages "$($BVM_INSTALL_DIR/bin/bvm-bin hidden-shell get-env-vars)"

  if [ ! -z "$bvm_binary_paths" ]
  then
    PATH="$bvm_binary_paths:$PATH"
  fi

  PATH="$BVM_INSTALL_DIR/shims:$PATH"
  export PATH
fi

# export the function to sub shells (does not work in sh, only bash)
export -f bvm
