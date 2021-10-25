& bvm install --use ../target/debug/args_test_util.json
# for some reason, double quotes within double quotes escaped with backticks wasn't even working with deno.exe
& deno eval 'console.log(`"hello`")'
& deno eval "console.log(2 != 3)"
& deno eval -p "JSON.stringify({})"
& deno eval "console.log(Deno.args[0])" lib=""
& deno eval "console.log(Deno.args[0])" -- lib=test,other
