#!/bin/sh

handle_env_messages()
{
  # * ADD\n<key>\n<value>
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
    else
      echo "Internal error. Unexpected output: $line"
      exit 1
    fi
  done
}

if [ -z "$BVM_INSTALL_DIR" ]
then
  echo "You must specify a BVM_INSTALL_DIR environment variable (ex. \`export BVM_INSTALL_DIR=\"$HOME/.bvm\"\`)."
else
  # use the bin directly since we haven't set the path yet
  bvm_binary_paths=$($BVM_INSTALL_DIR/bin/bvm-bin hidden-shell get-paths)

  handle_env_messages "$($BVM_INSTALL_DIR/bin/bvm-bin hidden-shell get-env-vars)"

  if [ -z "$bvm_binary_paths" ]
  then
    export PATH="$bvm_binary_paths:$PATH"
  fi

  export PATH="$BVM_INSTALL_DIR/bin:$BVM_INSTALL_DIR/shims:$PATH"
fi
