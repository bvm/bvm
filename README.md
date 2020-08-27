# bvm - Binary Version Manager

A version manager for all binaries.

![Demo using bvm command](demo.gif "Demo using bvm command")

NOTICE: This is a proof of concept and currently has no automated tests—extremely unstable. It is not recommended to use it yet as there will likely be many breaking changes.

## Goals

1. Seamless version selection based on current working directory.
2. Replace binary specific version manager tools.
3. No centralization—all urls and paths.
   - Allows for easily distributing approved binaries within an organization.
   - Easy for binary authors to distribute their applications.
4. Support completely different application binaries with the same command name.
5. Backwards compatibility loading project configuration files (once hitting 1.0)
6. **Works on Windows without needing WSL.**
7. Allows working with binaries already on the path (ex. `bvm use deno path`).

## Setup

1. For now, install `bvm` via cargo—`cargo install bvm`.
2. Manually add the shims folder as the first item on the path:
   - Windows: `C:\Users\<user-name>\AppData\Local\bvm\bvm\shims`
   - Mac/Linux: `~/.local/share/bvm/shims`
3. Add a _.bvmrc.json_ file to your project and specify the paths to the binary manifest files.
   ```jsonc
   {
     "binaries": [
       "https://bvm.land/deno/1.3.1.json",
       "https://bvm.land/dprint/0.9.1.json"
     ]
   }
   ```
   Or specify with checksums to ensure the remote files don't change:
   ```jsonc
   {
     "binaries": [
       "https://bvm.land/deno/1.3.1.json@8880bc307aba44b724b7d3ed56fa89165006bf1e5c85229441e3c209880b9534",
       "https://bvm.land/dprint/0.9.1.json@7fba741001eee1f02ec85faf7b0fed0244f9f1e57502f9e421220c3e4f98f98b"
     ]
   }
   ```
4. Run `bvm install`

## Commands

### `bvm install`

Downloads & installs the binaries in the current configuration file and associates them on the path with bvm.

### `bvm install [url]`

Installs a binary at the specified manifest file.

```
# Examples
bvm install https://bvm.land/deno/1.3.1.json
# optionally specify a checksum
bvm install https://bvm.land/deno/1.3.1.json@8880bc307aba44b724b7d3ed56fa89165006bf1e5c85229441e3c209880b9534
```

### `bvm use [binary-name or owner-name/binary-name] [version]`

Uses the specified binary name and version globally.

The binary and version must have been previously installed.

```
# Examples
bvm use deno 1.3.1
bvm use denoland/deno 1.3.1
bvm use name-stealer/deno 2.0.0
```

### `bvm use [binary-name or owner-name/binary-name] path`

Use the version of the binary that's installed on the path if it exists.

```
# Example
bvm use deno path
```

### `bvm resolve [binary name]`

Resolves the executable path of the specified binary based on the current working directory.

This command is used by the created shell/batch files (shims) to tell how to resolve the file.

```
# Example
bvm resolve deno
# on windows, outputs: C:\Users\<user>\AppData\Local\bvm\bvm\binaries\denoland\deno\1.3.1\deno.exe
```

### `bvm uninstall [binary-name or owner-name/binary-name] [version]`

Uninstalls the specified binary version.

```
# Examples
bvm uninstall deno 1.2.0
bvm uninstall denoland/deno 1.3.1
bvm uninstall name-stealer/deno 2.0.0
```

## Binary manifest file

At the moment, it looks like this (will add architecture specific stuff later):

```json
{
  "schemaVersion": 1,
  "name": "deno",
  "owner": "denoland",
  "version": "1.3.1",
  "windows": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-pc-windows-msvc.zip",
    "checksum": "6ba068e517a55dd33abd60e74c38aa61ef8f45a0774578761be0107fafc3758b",
    "binaryPath": "deno.exe",
    "postExtract": "# this is the post extract script where you can run some commands if necessary to cause additional setup"
  },
  "linux": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-unknown-linux-gnu.zip",
    "checksum": "ef3a8740bdceab105808c91cfb918c883a23defb6719b9c511e2be30d5bfdc01",
    "binaryPath": "deno"
  },
  "mac": {
    "archive": "https://github.com/denoland/deno/releases/download/v1.3.1/deno-x86_64-apple-darwin.zip",
    "checksum": "b1bc5de79b71c3f33d0151486249d088f5f5604126812dc55b1dd21b28704d8a",
    "binaryPath": "deno"
  }
}
```

## Future improvements

Low effort:

1. `bvm list` - Lists the installed binaries.
2. `bvm clear-url-cache` - Clear the url caches, but not the binary caches.
3. Ability to get a specific version of a binary when using `bvm resolve` (ex. `bvm resolve deno 1.3.1`)
4. Ability to specify pre & post install commands in the configuration file.
5. Require `--force` on `bvm install <url>` if already installed.
6. Command aliases in the configuration file.
   ```jsonc
   {
     "binaries": [{
       "manifest": "https://bvm.land/deno/1.3.1.json",
       "alias": "deno-1.3.1"
     }]
   }
   ```
7. Add `bvm init` to create a bare configuration file in the current directory.
8. Add `bvm lock` to update the configuration file urls with checksums.

Medium effort:

1. Ability to specify a range of supported versions in _.bvmrc.json_ to reduce the number of downloaded binaries:
   ```jsonc
   {
     "binaries": [{
       "manifest": "https://bvm.land/deno/1.3.1.json",
       "version": "^1.3.0"
     }]
   }
   ```
2. Support for file paths everywhere in addition to urls.
3. Ability to easily create and remove aliases (ex. `deno2`)
   - These should be associated with the binary they alias so when you uninstall the binary it deletes the alias.
4. Ability to execute a specific version of an executable one time. `bvm exec deno 1.2.0 -V` or perhaps at the shim level `deno -V --bvm-use-version 1.2.0`... or maybe this should use `bvm resolve` somehow.

Large effort:

1. Some way for binaries to specify all their version numbers and the ability to get their latest. ~~I'm thinking each binary manifest file may have a url to a global binary manifest file where all that data is stored.~~ I think this should be explicit as people will have to trust the source. They could add "binary list" files to their individual CLI tools then install via `bvm install [binary name] [version]` or just `bvm install [binary name]`.
2. `bvm upgrade <binary name>` - Upgrade to the latest version (requires a "binary list" to be set—not implemented)
3. Support downstream binary dependencies (should also support a range of dependencies).

Probably unnecessary complexity:

1. `bvm use <url>` - To use a specific version of a binary globally via a url.
2. ~~Something similar to `npm run <script-name>`? Or is that out of scope?~~ Yes. I think there should be another tool people can install with bvm that does this. This tool should be very simple. There should definitely be pre and post install scripts though.
3. `bvm use <binary name> <executable file path>` for using the executable at the specified file path.
4. Ability to purge any binaries that haven't been run for X days.
