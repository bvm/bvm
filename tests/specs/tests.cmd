@echo off

CALL bvm install --use https://bvm.land/deno/1.15.2.json@affc933ab2513eb554f3188dee15bee19b0c97d171138384d31b611dafe679a2
CALL deno eval "console.log(\"hello\")"
CALL deno eval "console.log(2 != 3)"
REM cmd supports no quotes here, but powershell and cmd both require them
CALL deno eval -p JSON.stringify({})
CALL deno eval -p "JSON.stringify({})"
CALL deno eval "console.log(Deno.args[0])" lib=""
CALL deno eval "console.log(Deno.args[0])" -- lib=test,other
