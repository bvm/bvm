#!/bin/bash

cargo build --package args_test_util
zip -r target/debug/args_test_util.zip target/debug/args_test_util
checksum=$(shasum -a 256 target/debug/args_test_util.zip | awk '{print $1}')
file_text="{\
  \"schemaVersion\": 1,\
  \"name\": \"args_test_util\",\
  \"owner\": \"bvm\",\
  \"version\": \"0.1.0\",\
  \"description\": \"Test Utility\",\
  \"darwin-x86_64\": {\
    \"path\": \"./args_test_util.zip\",\
    \"type\": \"zip\",\
    \"checksum\": \"$checksum\",\
    \"commands\": [{\
      \"name\": \"args_test_util\",\
      \"path\": \"args_test_util.exe\"\
    }]\
  }\
}"
cat $file_text >> target/debug/args_test_util.json
