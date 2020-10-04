cargo build
cp target/debug/bvm-bin ~/.bvm/bin/bvm-bin
cp bvm.sh ~/.bvm/bin/bvm
cp bvm-init.sh ~/.bvm/bin/bvm-init
BVM_INSTALL_DIR=$HOME/.bvm
export BVM_INSTALL_DIR
. ~/.bvm/bin/bvm-init
~/.bvm/bin/bvm-bin unix-install
