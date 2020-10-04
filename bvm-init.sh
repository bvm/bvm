#!/bin/sh

if [ -z "$BVM_INSTALL_DIR" ]
then
  echo "You must specify a BVM_INSTALL_DIR environment variable (ex. \`export BVM_INSTALL_DIR=\"$HOME/.bvm\"\`)."
  exit 1
fi

. $BVM_INSTALL_DIR/bin/bvm

# use the bin directly since we haven't set the path yet
bvm_binary_paths=$($BVM_INSTALL_DIR/bin/bvm-bin hidden get-paths)

bvm_handle_env_messages "$($BVM_INSTALL_DIR/bin/bvm-bin hidden get-env-vars)"

if [ ! -z "$bvm_binary_paths" ]
then
  PATH="$bvm_binary_paths:$PATH"
fi

# todo: use bvm_install_dir here (need to update get_shim_dir.rs)
PATH="$HOME/.bvm/shims:$PATH"
export PATH

# export the function to sub shells (does not work in sh, only bash)
export -f bvm
