#!/bin/sh
bvm-bin "$@"

if [ "$1" = "install" ] || [ "$1" = "uninstall" ] || [ "$1" = "use" ]
then
  new_path=$(bvm-bin hidden-shell get-new-path "$PATH")
  if [ "$PATH" != "$new_path" ]
  then
    export PATH=$new_path
    bvm-bin hidden-shell clear-pending-changes
  fi
fi
