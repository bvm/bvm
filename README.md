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
3. Add a _.gvmrc.json_ file to your project and specify the paths to the binary manifest files.
   ```jsonc
   {
     "binaries": [
       // these don't exist anywhere at the moment except on my machine (again, proof of concept)
       "http://localhost:8000/deno-1.3.1.json",
       "http://localhost:8000/dprint-0.9.0.json"
     ]
   }
   ```
4. Run `gvm install`

## Commands

### `gvm install`

Adds the binaries in the current configuration file to the path then downloads & installs them.

### `gvm install <url>`

Installs a binary at the specified manifest file.

### `gvm use [binary name] [version]`

Uses the specified binary name and version globally.

The binary and version must have been previously installed.

### `gvm resolve [binary name]`

Resolves the executable path of the specified binary using the provided arguments based on the current working directory.

This command is used by the created shell/batch files to tell how to resolve the file.

### `gvm uninstall [binary name] [version]`

Uninstalls the specified binary name and version.

## Binary manifest file

At the moment, it looks like this:

```json
{
  "schemaVersion": 1,
  "name": "deno",
  "group": "denoland",
  "version": "1.3.1",
  "windows": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-pc-windows-msvc.zip",
    "binaryPath": "deno.exe",
    "postExtract": "# this is the post extract script where you can run some commands if necessary to cause additional setup"
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

## Future improvements

1. Ability to specify a range of supported versions in _.gvmrc.json_ to reduce the number of downloaded binaries:
   ```jsonc
   {
     "binaries": [
       // I don't know... maybe something like this
       {
         "manifest": "http://localhost:8000/deno-1.3.1.json",
         "version": "^1.3.0"
       }
     ]
   }
   ```
2. Support for file paths everywhere in addition to urls.
3. `gvm use <url>` - To use a specific version of a binary globally via a url.
4. Something similar to `npm run <script-name>`? Or is that out of scope?
5. Ability to specify pre & post install commands in the configuration file (ties into #4 maybe... might be better to make it separate though)
6. Ability to purge any binaries that haven't been run for X days.
7. Some way for binaries to specify all their version numbers and the ability to get their latest. I'm thinking each binary manifest file may have a url to a global binary manifest file where all that data is stored.
8. Checksums on paths to ensure downstream binaries stay constant.
9. `gvm list` - Lists the installed binaries.
10. `gvm upgrade <binary name>` - Upgrade to the latest version (requires binary manifest file to specify a global manifest file)
11. Support downstream binary dependencies.
12. Ability to run a specific version of a binary when using `gvm resolve`
13. Ability to easily create aliases (ex. `deno2`)
14. Require `--force` on `gvm install` if already installed.
15. `gvm clear-url-cache` - Clear the url caches, but not the binary caches.

## Goals

1. Seamless version selection.
2. Replace binary specific version manager tools.
3. No centralization—all urls and paths.
   - Allows for easily distributing approved binaries within an organization.
   - Easy for binary authors to distribute their applications.
4. Support different binaries with the same name.
5. Backwards compatibility (once hitting 1.0)
