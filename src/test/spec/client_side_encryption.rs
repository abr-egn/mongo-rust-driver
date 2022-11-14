use tokio::sync::RwLockWriteGuard;

use crate::test::LOCK;

use super::{run_spec_test_with_path, run_unified_format_test_filtered, unified_runner::TestCase};

#[cfg_attr(feature = "tokio-runtime", tokio::test(flavor = "multi_thread"))]
#[cfg_attr(feature = "async-std-runtime", async_std::test)]
async fn run() {
    let _guard: RwLockWriteGuard<()> = LOCK.run_exclusively().await;
    run_spec_test_with_path(&["client-side-encryption", "unified"], |path, test| {
        run_unified_format_test_filtered(path, test, spec_predicate)
    })
    .await;
}

#[allow(unused_variables)]
fn spec_predicate(test: &TestCase) -> bool {
    #[cfg(not(feature = "openssl-tls"))]
    {
        if test.description == "create datakey with KMIP KMS provider" {
            crate::test::log_uncaptured(format!(
                "Skipping {:?}: KMIP test requires openssl-tls",
                test.description
            ));
            return false;
        }
    }
    true
}
