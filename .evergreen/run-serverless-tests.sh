#!/usr/bin/env bash

set -o errexit

FEATURE_FLAGS="zstd-compression,snappy-compression,zlib-compression"
DEFAULT_FEATURES=""

if [ "$ASYNC_RUNTIME" = "async-std" ]; then
    FEATURE_FLAGS="${FEATURE_FLAGS},async-std-runtime"
    DEFAULT_FEATURES="--no-default-features"
elif [ "$ASYNC_RUNTIME" != "tokio" ]; then
    echo "invalid async runtime: ${ASYNC_RUNTIME}" >&2
    exit 1
fi

. ~/.cargo/env

if [ "$SINGLE_THREAD" = true ]; then
	OPTIONS="-- --test-threads=1"
fi

echo "cargo test options: ${DEFAULT_FEATURES} --features $FEATURE_FLAGS ${OPTIONS}"

cargo_test() {
    SERVERLESS="serverless" \
        cargo test ${DEFAULT_FEATURES} --features $FEATURE_FLAGS $1 $OPTIONS
}

cargo_test test::spec::crud
cargo_test test::spec::retryable_reads
cargo_test test::spec::retryable_writes
cargo_test test::spec::versioned_api
cargo_test test::spec::sessions
cargo_test test::spec::transactions
cargo_test test::spec::load_balancers
cargo_test test::cursor