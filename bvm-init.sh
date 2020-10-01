#!/bin/sh
if [ -z "$BVM_INSTALL_DIR" ]
then
  echo "You must specify a BVM_INSTALL_DIR environment variable (ex. \`export BVM_INSTALL_DIR=\"$HOME/.bvm\"\`)."
else
  # use the bin directly since we haven't set the path yet
  bvm_binary_paths=$($BVM_INSTALL_DIR/bin/bvm-bin hidden-shell get-paths)

  if [ -z "$bvm_binary_paths" ]
  then
    export PATH="$bvm_binary_paths:$PATH"
  fi

  export PATH="$BVM_INSTALL_DIR/bin:$BVM_INSTALL_DIR/shims:$PATH"
fi
