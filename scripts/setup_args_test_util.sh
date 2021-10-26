#!/bin/bash

cargo build --package args_test_util
mkdir -p temp
copy target/debug/args_test_util temp/args_test_util
cd temp
zip -r args_test_util.zip args_test_util
checksum=$(shasum -a 256 args_test_util.zip | awk '{print $1}')
file_text=$(cat << EOF
{
  "schemaVersion": 1,
  "name": "args_test_util",
  "owner": "bvm",
  "version": "0.1.0",
  "description": "Test Utility",
  "darwin-x86_64": {
    "path": "./args_test_util.zip",
    "type": "zip",
    "checksum": "$checksum",
    "commands": [{
      "name": "args_test_util",
      "path": "args_test_util"
    }]
  },
  "linux-x86_64": {
    "path": "./args_test_util.zip",
    "type": "zip",
    "checksum": "$checksum",
    "commands": [{
      "name": "args_test_util",
      "path": "args_test_util"
    }]
  }
}
EOF
)

echo $file_text > args_test_util.json
