#!/bin/sh

if [[ -z "$BVM_BIN_PATH" ]]; then
  # checking $0 is not reliable if this script is being called from another
  # script, as $0 will be the initial script, so try to define this
  # environment variable when initially sourced (perhaps this is unnecessary?)
  export $BVM_BIN_PATH="$(dirname "$(readlink -f "$0")")/bvm-bin"
fi

bvm_handle_env_messages()
{
  # * ADD\n<key>\n<value>
  # * REMOVE\n<key>
  # * EXEC\n<path>
  local message_step
  message_step=0
  local bvm_var_name
  local IFS
  local lines
  lines=$1
  shift
  IFS='
'
  for line in $lines
  do
    if [ "$message_step" = "0" ]
    then
      if [ "$line" = "ADD" ]
      then
        message_step=1
      elif [ "$line" = "REMOVE" ]
      then
        message_step=10
      elif [ "$line" = "EXEC" ]
      then
        message_step=20
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
      eval "$bvm_var_name"=\"\$line\"
      export $bvm_var_name

      message_step=0
    elif [ "$message_step" = "10" ] # removing, get var value
    then
      eval "$line"=\"\"
      export $line
      unset "$line"
      message_step=0
    elif [ "$message_step" = "20" ] # exec
    then
      $line "$@" || { return $?; }
    else
      echo "Internal error. Unexpected output: $line"
      return 1
    fi
  done
}

bvm()
{
  if [ "$1" = "exec-command" ]
  then
    local bvm_exec_command
    bvm_exec_command=$2
    # todo: implement the third fallback argument somehow
    shift 3

    # use a sub shell to prevent exporting variables
    (
      bvm_handle_env_messages "$($BVM_BIN_PATH hidden resolve-command "$bvm_exec_command")" "$@" || { return $?; }
    )

    return $?;

  elif [ "$1" = "exec" ]
  then
    # Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
    local bvm_exec_name
    local bvm_exec_version
    local bvm_exec_command
    local bvm_has_command
    bvm_exec_name=$2
    bvm_exec_version=$3
    bvm_exec_command=$4
    bvm_has_command=$($BVM_BIN_PATH hidden has-command "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { return $?; }

    if [ "$bvm_has_command" = "false" ]
    then
      bvm_exec_command=$bvm_exec_name
    fi

    local bvm_executable_path
    bvm_executable_path=$($BVM_BIN_PATH hidden get-exec-command-path "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { return $?; }

    if [ "$bvm_has_command" = "false" ]
    then
      shift 3
    else
      shift 4
    fi

    # use a sub shell to prevent exporting variables
    (
      bvm_handle_env_messages "$($BVM_BIN_PATH hidden get-exec-env-changes "$bvm_exec_name" "$bvm_exec_version")" || { return $?; }

      $bvm_executable_path "$@"
    )
    return $?
  fi

  $BVM_BIN_PATH "$@"

  if [ "$1" = "install" ] || [ "$1" = "uninstall" ] || [ "$1" = "use" ]
  then
    local pending_changes
    pending_changes=$($BVM_BIN_PATH hidden get-pending-env-changes)
    if [ ! -z "$pending_changes" ]
    then
      bvm_handle_env_messages "$pending_changes" || { return $?; }
      $BVM_BIN_PATH hidden clear-pending-env-changes || { return $?; }
    fi
  fi
}
