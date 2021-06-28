#!/bin/sh
# Adapted from Deno's install script (https://github.com/denoland/deno_install/blob/master/install.sh)

set -e

windows_target="x86_64-pc-windows-msvc"

case $(uname -s) in
Darwin) target="x86_64-apple-darwin" ;;
*_NT-*) target="$windows_target" ;;
*) target="x86_64-unknown-linux-gnu" ;;
esac

if [ $(uname -m) != "x86_64" ]; then
  echo "Unsupported architecture $(uname -m). Only x64 binaries are available."
  exit
fi

if [ $# -eq 0 ]; then
  bvm_uri="https://github.com/dsherret/bvm/releases/latest/download/bvm-${target}.zip"
else
  bvm_uri="https://github.com/dsherret/bvm/releases/download/${1}/bvm-${target}.zip"
fi

bvm_install="${BVM_INSTALL_DIR:-$HOME/.bvm}"
BVM_INSTALL_DIR="$bvm_install"
bin_dir="$bvm_install/bin"
exe="$bin_dir/bvm"

if [ ! -d "$bin_dir" ]; then
  mkdir -p "$bin_dir"
fi

# stop any running bvm processes
pkill -9 "bvm" || true

# download and install
curl --fail --location --progress-bar --output "$exe.zip" "$bvm_uri"
cd "$bin_dir"
unzip -o "$exe.zip"
rm "$exe.zip"

if [ "$target" = "$windows_target" ]
then
  "$exe-bin.exe" hidden windows-install

  PATH="$APPDATA/bvm/shims:$bin_dir:$PATH"
  export PATH

  echo "bvm was installed successfully to $exe"
  echo "Run 'bvm --help' to get started"
else
  chmod +x "$exe"
  chmod +x "$exe-bin"
  chmod +x "$exe-init"

  "$exe-bin" hidden unix-install
  . $exe-init

  echo "bvm was installed successfully to $exe"
  if command -v bvm >/dev/null; then
    echo "Run 'bvm --help' to get started"
  else
    case $SHELL in
    /bin/zsh) shell_profile=".zshrc" ;;
    *) shell_profile=".bash_profile" ;;
    esac
    echo "Manually add the following to your \$HOME/$shell_profile (or similar)"
    echo ""
    echo "export BVM_INSTALL_DIR=\"$bvm_install\""
    echo ". \"\$BVM_INSTALL_DIR/bin/bvm-init\""
    echo ""
    echo "Run '$exe --help' to get started"
  fi
fi
