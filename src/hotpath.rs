#[cfg(feature = "hotpath")]
const HOTPATH_REPORT_ENTRYPOINT: &str = "seagrass";
#[cfg(feature = "hotpath")]
const DEFAULT_REPORT_OUTPUT_PATH: &str = "/dev/stderr";
#[cfg(feature = "hotpath")]
const HOTPATH_PERCENTILES: &[f64] = &[50.0, 95.0, 99.0];
#[cfg(feature = "hotpath")]
const UNLIMITED_FUNCTIONS: usize = 0;

#[cfg(feature = "hotpath")]
pub(crate) fn install_guard() -> hotpath::HotpathGuard {
    let output_path = std::env::var("HOTPATH_OUTPUT_PATH")
        .unwrap_or_else(|_| DEFAULT_REPORT_OUTPUT_PATH.to_string());

    hotpath::HotpathGuardBuilder::new(HOTPATH_REPORT_ENTRYPOINT)
        .percentiles(HOTPATH_PERCENTILES)
        .functions_limit(UNLIMITED_FUNCTIONS)
        .output_path(output_path)
        .build()
}

#[cfg(not(feature = "hotpath"))]
pub(crate) struct DisabledGuard;

#[cfg(not(feature = "hotpath"))]
pub(crate) fn install_guard() -> DisabledGuard {
    DisabledGuard
}

#[macro_export]
macro_rules! measure_hotpath_block {
    ($label:literal, $block:block) => {{
        #[cfg(feature = "hotpath")]
        {
            hotpath::measure_block!($label, $block)
        }
        #[cfg(not(feature = "hotpath"))]
        {
            $block
        }
    }};
}
