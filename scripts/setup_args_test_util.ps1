$ErrorActionPreference = "Stop"

cargo build --package args_test_util
Compress-Archive -CompressionLevel Optimal -Force -Path target/debug/args_test_util.exe -DestinationPath target/debug/args_test_util.zip
$file_hash=(Get-FileHash target/debug/args_test_util.zip).Hash
$file_text=@'
{{
  "schemaVersion": 1,
  "name": "args_test_util",
  "owner": "bvm",
  "version": "0.1.0",
  "description": "Test Utility",
  "windows-x86_64": {{
    "path": "./args_test_util.zip",
    "type": "zip",
    "checksum": "{0}",
    "commands": [{{
      "name": "args_test_util",
      "path": "args_test_util.exe"
    }}]
  }}
}}
'@ -f $file_hash.ToLower()
echo $file_text | Out-File -FilePath target/debug/args_test_util.json
