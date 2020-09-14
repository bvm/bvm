#!/bin/sh
if [ -z "$BVM_INSTALL_DIR" ]
then
  echo "You must specify a BVM_INSTALL_DIR environment variable (ex. \`export BVM_INSTALL_DIR=\"$HOME/.bvm\"\`)."
else
  bvm_binary_paths=$($BVM_INSTALL_DIR/bvm hidden-shell get-paths)
  if [ -z "$bvm_binary_paths" ]
  then
  export PATH="$bvm_binary_paths;$PATH"
  fi

  if [ -z "$BVM_LOCAL_DATA_DIR" ]
  then
  export BVM_LOCAL_DATA_DIR="$BVM_INSTALL_DIR"
  fi
  if [ -z "$BVM_DATA_DIR" ]
  then
  export BVM_DATA_DIR="$BVM_INSTALL_DIR"
  fi

  export PATH="$BVM_INSTALL_DIR/bin:$BVM_DATA_DIR/shims:$PATH"
fi
