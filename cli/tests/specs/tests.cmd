@echo off

CALL bvm install --use ../../../target/debug/args_test_util.json
CALL args_test_util "console.log(\"hello\")"
CALL args_test_util "console.log(2 != 3)"
REM cmd supports no quotes here, but powershell and cmd both require them
CALL args_test_util JSON.stringify({})
CALL args_test_util "JSON.stringify({})"
CALL args_test_util lib=""
CALL args_test_util lib=test,other
