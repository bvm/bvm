#!/bin/sh

if [ -z "$BVM_INSTALL_DIR" ]
then
  BVM_INSTALL_DIR="$HOME/.bvm"
fi

. $BVM_INSTALL_DIR/bin/bvm

# use the bin directly since we haven't set the path yet
bvm_binary_paths=$($BVM_INSTALL_DIR/bin/bvm-bin hidden get-paths)

bvm_handle_env_messages "$($BVM_INSTALL_DIR/bin/bvm-bin hidden get-env-vars)"

if [ ! -z "$bvm_binary_paths" ]
then
  PATH="$bvm_binary_paths:$PATH"
fi

PATH="$BVM_INSTALL_DIR/shims:$PATH"
export PATH
