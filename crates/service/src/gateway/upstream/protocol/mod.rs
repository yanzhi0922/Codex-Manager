pub(super) mod aggregate_api;
pub(super) mod azure_openai;

pub(in crate::gateway) fn clear_runtime_state() {
    aggregate_api::clear_runtime_state();
}
