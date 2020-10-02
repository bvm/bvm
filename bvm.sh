#!/bin/sh
bvm_bin="bvm-bin"

handle_env_messages()
{
  # * ADD\n<key>\n<value>
  # * REMOVE\n<key>
  message_step=0
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
        exit 1
      fi
    elif [ "$message_step" = "1" ] # adding, get var name
    then
      bvm_var_name="$line"
      message_step=2
    elif [ "$message_step" = "2" ] # adding, get var value
    then
      export $bvm_var_name="$line"
      message_step=0
    elif [ "$message_step" = "10" ] # removing, get var value
    then
      unset "$line"
      message_step=0
    else
      echo "Internal error. Unexpected output: $line"
      exit 1
    fi
  done
}

if [ "$1" = "update-environment" ]
then
  bvm_new_environment=$bvm_bin hidden-shell get-pending-env-changes
  if [ $? -eq 0 ]
  then
    handle_env_messages "$bvm_new_environment"
    $bvm_bin hidden-shell clear-pending-env-changes
  fi
  exit $?
fi

if [ "$1" = "exec" ]
then
  # Format: bvm exec [name-selector] [version-selector] [command-name] [...args]
  bvm_exec_name=$2
  bvm_exec_version=$3
  bvm_exec_command=$4
  bvm_has_command=$($bvm_bin hidden-shell has-command "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { exit $?; }

  if [ "$bvm_has_command" = "false" ]
  then
    bvm_exec_command=$bvm_exec_name
  fi

  bvm_executable_path=$($bvm_bin hidden-shell get-exec-command-path "$bvm_exec_name" "$bvm_exec_version" "$bvm_exec_command") || { exit $?; }

  if [ "$bvm_has_command" = "false" ]
  then
    shift 3
  else
    shift 4
  fi
  bvm_exec_args="$@"

  handle_env_messages "$($bvm_bin hidden-shell get-exec-env-changes "$bvm_exec_name" "$bvm_exec_version")"

  $bvm_executable_path $bvm_exec_args
  exec_exit_code=$?

  handle_env_messages "$($bvm_bin hidden-shell get-post-exec-env-changes "$bvm_exec_name" "$bvm_exec_version")"

  exit $exec_exit_code
fi

$bvm_bin "$@"

if [ "$1" = "install" ] || [ "$1" = "uninstall" ] || [ "$1" = "use" ]
then
  pending_changes=$($bvm_bin hidden-shell get-pending-env-changes)
  if [ ! -z "$pending_changes" ]
  then
    echo 'The environment has changed. Update your environment by running the following command:'
    echo 'source bvm update-environment'
  fi
fi
