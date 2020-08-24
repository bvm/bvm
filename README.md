# gvm - Global Version Manager

A version manager for all binaries.

![Demo using gvm command](demo.gif "Demo using gvm command")

## Why?

I couldn't find a solution like this that also worked on Windows. Do you know something? If so, please stop me so I don't waste my time.

## Setup

It's not recommended to try this out yet as there are no public binary manifest files, but if you want to:

1. For now, install `gvm` via cargo—`cargo install gvm`.
2. Manually add the binary folder to the path:
   - Windows: `C:\Users\<user-name>\AppData\Local\gvm\gvm\bin`
   - Mac/Linux: `~/.local/share/gvm/bin`
3. Add a _.gvmrc.json_ file to your project and specify the binary names and paths to the binary manifest files.
   ```jsonc
   {
     "binaries": {
       // these don't exist anywhere at the moment (again, proof of concept)
       "deno": "http://localhost:8000/deno-1.3.1.json",
       "dprint": "http://localhost:8000/dprint-0.9.0.json"
     }
   }
   ```
4. Run `gvm install`

## Commands

### `gvm install`

Adds the binaries in the current configuration file to the path then downloads & installs them.

### `gvm use <binary name> <version>`

Uses the specified binary name and version globally.

The binary and version must have been previously installed.

### `gvm run [binary-name] [...args]`

Runs the specified binary using the provided arguments based on the current configuration file.

Will download & install the binary if it hasn't been installed.

This is what `[binary-name] [...args]` internally uses to run the correct binary.

## Binary manifest file

At the moment, it looks like this:

```json
{
  "schemaVersion": 1,
  "name": "deno",
  "version": "1.3.1",
  "windows": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-pc-windows-msvc.zip",
    "binaryPath": "deno.exe"
  },
  "linux": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-unknown-linux-gnu.zip",
    "binaryPath": "deno"
  },
  "mac": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-apple-darwin.zip",
    "binaryPath": "deno"
  }
}
```

Obviously this needs to be improved.

## Future improvements

1. Ability to specify a range of supported versions in _.gvmrc.json_ to reduce the number of downloaded binaries:
   ```jsonc
   {
     "binaries": {
       // I don't know... something like this
       "deno": {
         "version": "^1.3.0",
         "download": "http://localhost:8000/deno-1.3.1.json"
       }
     }
   }
   ```
2. Support for file paths everywhere in addition to urls.
3. `gvm install <url>` - To install a binary at the specified url.
4. `gvm use <url>` - To use a specific version of a binary globally via a url.
5. Ability to specify pre & post install commands in the configuration file.
6. Something similar to `npm run <script-name>`.
7. Ability to purge any binaries that haven't been run for X days.
8. Some way for binaries to specify all their version numbers and the ability to get their latest. I'm thinking each binary manifest file may have a url to a global manifest file where all that data is stored.
9. `gvm uninstall <binary name> <version>` or `gvm uninstall <url>`
10. Ability to run some setup commands after extracting binary zip file.
11. Checksums on paths to ensure downstream binaries stay constant.
12. `gvm list` - Lists the installed binaries.
13. `gvm upgrade <binary name>` - Upgrade to the latest version (requires binary manifest file to specify a global manifest file)
14. Support downstream binary dependencies.

## Goals

1. Seamless version selection.
2. Replace binary specific version manager tools.
3. No centralization—all urls and paths.
   - Allows for easily distributing approved binaries within an organization.
   - Easy for binary authors to distribute their applications.
4. Support different binaries with the same name.
