@echo off

CALL bvm install --use ../target/debug/args_test_util.json
CALL deno eval "console.log(\"hello\")"
CALL deno eval "console.log(2 != 3)"
REM cmd supports no quotes here, but powershell and cmd both require them
CALL deno eval -p JSON.stringify({})
CALL deno eval -p "JSON.stringify({})"
CALL deno eval "console.log(Deno.args[0])" lib=""
CALL deno eval "console.log(Deno.args[0])" -- lib=test,other
