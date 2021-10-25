#!/bin/sh

bvm install --use https://bvm.land/deno/1.15.2.json@affc933ab2513eb554f3188dee15bee19b0c97d171138384d31b611dafe679a2
deno eval "console.log(\"hello\")"
deno eval "console.log(2 != 3)"
deno eval -p "JSON.stringify({})"
deno eval "console.log(Deno.args[0])" lib=""
deno eval "console.log(Deno.args[0])" -- lib=test,other