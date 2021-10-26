#!/bin/bash
set -e

# setup and source the bvm function
chmod +x target/debug/home_dir/bin/bvm.sh
export BVM_BIN_PATH=$(realpath target/debug/home_dir/bin/bvm-bin)
. target/debug/home_dir/bin/bvm.sh

bvm install --use target/debug/args_test_util.json
args_test_util "console.log(\"hello\")"
args_test_util "console.log(2 != 3)"
args_test_util "JSON.stringify({})"
args_test_util lib=""
args_test_util lib=test,other