#!/bin/bash
set -e

# setup and source the bvm function
chmod +x ../../../temp/home_dir/bin/bvm.sh
export BVM_BIN_PATH=$(../../../temp/home_dir/bin/bvm-bin)
. ../../../temp/home_dir/bin/bvm.sh

bvm install --use ../../../temp/args_test_util.json
args_test_util "console.log(\"hello\")"
args_test_util "console.log(2 != 3)"
args_test_util "JSON.stringify({})"
args_test_util lib=""
args_test_util lib=test,other