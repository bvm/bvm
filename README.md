# bvm - Binary Version Manager

A version manager for all binaries.

![Demo using bvm command](demo.gif "Demo using bvm command")

## Goals

1. Seamless version selection.
2. Replace binary specific version manager tools.
3. No centralization—all urls and paths.
   - Allows for easily distributing approved binaries within an organization.
   - Easy for binary authors to distribute their applications.
4. Support different binaries with the same name.
5. Backwards compatibility loading project configuration files (once hitting 1.0)
6. Works on Windows without needing WSL.

## Setup

It's not recommended to try this out yet as there are no public binary manifest files, but if you want to:

1. For now, install `bvm` via cargo—`cargo install bvm`.
2. Manually add the binary folder as the first item on the path:
   - Windows: `C:\Users\<user-name>\AppData\Local\bvm\bvm\bin`
   - Mac/Linux: `~/.local/share/bvm/bin`
3. Add a _.bvmrc.json_ file to your project and specify the paths to the binary manifest files.
   ```jsonc
   {
     "binaries": [
       // these don't exist anywhere at the moment except on my machine (again, proof of concept)
       "https://bvm.land/deno/1.3.1.json",
       "https://bvm.land/dprint/0.9.0.json"
     ]
   }
   ```
4. Run `bvm install`

## Commands

### `bvm install`

Adds the binaries in the current configuration file to the path then downloads & installs them.

### `bvm install <url>`

Installs a binary at the specified manifest file.

### `bvm use [binary-name or group-name/binary-name] [version]`

Uses the specified binary name and version globally.

The binary and version must have been previously installed.

### `bvm resolve [binary name]`

Resolves the executable path of the specified binary based on the current working directory.

This command is used by the created shell/batch files (shims) to tell how to resolve the file.

### `bvm uninstall [binary-name or group-name/binary-name] [version]`

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

1. Ability to specify a range of supported versions in _.bvmrc.json_ to reduce the number of downloaded binaries:
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
3. Ability to specify pre & post install commands in the configuration file.
4. Ability to purge any binaries that haven't been run for X days.
5. Some way for binaries to specify all their version numbers and the ability to get their latest. ~~I'm thinking each binary manifest file may have a url to a global binary manifest file where all that data is stored.~~ I think this should be explicit as people will have to trust the source. They could add "binary list" files to their individual CLI tools then install via `bvm install [binary name] [version]` or just `bvm install [binary name]`.
6. Checksums on paths to ensure downstream binaries stay constant.
7. `bvm list` - Lists the installed binaries.
8. `bvm upgrade <binary name>` - Upgrade to the latest version (requires binary manifest file to specify a global manifest file)
9. Support downstream binary dependencies.
10. Ability to get a specific version of a binary when using `bvm resolve` (ex. `bvm resolve deno 1.3.1`)
11. Ability to easily create and remove aliases (ex. `deno2`)
    - These should be associated with the binary they alias so when you uninstall the binary it deletes the alias.
12. Require `--force` on `bvm install <url>` if already installed.
13. `bvm clear-url-cache` - Clear the url caches, but not the binary caches.
14. Ability to execute a specific version of an executable one time. `bvm exec deno 1.2.0 -V` or perhaps at the shim level `deno -V --bvm-use-version 1.2.0`... or maybe this should use `bvm resolve` somehow.

Probably unnecessary complexity:

1. `bvm use <url>` - To use a specific version of a binary globally via a url.
2. ~~Something similar to `npm run <script-name>`? Or is that out of scope?~~ Yes. I think there should be another tool people can install with bvm that does this. This tool should be very simple. There should definitely be pre and post install scripts though.
3. `bvm use <binary name> <executable file path>` for using the executable at the specified file path.
