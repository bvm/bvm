#!/bin/bash
set -e

# setup and source the bvm function
root_dir=$1
chmod +x $root_dir/temp/home_dir/bin/bvm.sh
export BVM_INSTALL_DIR=$root_dir/temp/home_dir
. $BVM_INSTALL_DIR/bin/bvm.sh

bvm install --use $root_dir/temp/args_test_util.json
args_test_util "console.log(\"hello\")"
args_test_util "console.log(2 != 3)"
args_test_util "JSON.stringify({})"
args_test_util lib=""
args_test_util lib=test,other