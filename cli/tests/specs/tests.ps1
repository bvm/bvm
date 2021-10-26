& bvm install --use target/debug/args_test_util.json
# for some reason, double quotes within double quotes escaped with backticks wasn't working even with the exe
# & args_test_util 'console.log(`"hello`")'
& args_test_util "console.log(2 != 3)"
& args_test_util "JSON.stringify({})"
& args_test_util lib=""
& args_test_util lib=test,other
