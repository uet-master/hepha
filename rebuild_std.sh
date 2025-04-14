#!/bin/bash
#Copyright (c) Facebook, Inc. and its affiliates.

# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

# Use this script to update the seed summary store in hepha/binaries/summary_store.tar

# Exit immediately if a command exits with a non-zero status.
set -e

cargo build --no-default-features

# build the hepha-standard-contracts crate
touch standard_contracts/src/lib.rs
cargo build --lib -p hepha-standard-contracts
touch standard_contracts/src/lib.rs
RUSTC_WRAPPER=target/debug/hepha RUST_BACKTRACE=1 HEPHA_LOG=warn HEPHA_START_FRESH=true HEPHA_SHARE_PERSISTENT_STORE=true HEPHA_FLAGS="--diag=paranoid" cargo build --lib -p hepha-standard-contracts

# collect the summary store into a tar file
cd target/debug/deps
tar -c -f ../../../binaries/summary_store.tar .summary_store.sled
cd ../../..