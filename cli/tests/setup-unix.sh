cargo build
cp target/debug/bvm-bin ~/.bvm/bin/bvm-bin
cp cli/bvm.sh ~/.bvm/bin/bvm
cp cli/bvm-init.sh ~/.bvm/bin/bvm-init
BVM_INSTALL_DIR=$HOME/.bvm
export BVM_INSTALL_DIR
. ~/.bvm/bin/bvm-init
~/.bvm/bin/bvm-bin hidden unix-install
