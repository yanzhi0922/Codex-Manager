use codexmanager_core::rpc::types::{JsonRpcRequest, JsonRpcResponse};

use crate::platforms;

pub(super) fn try_handle(req: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    let result = match req.method.as_str() {
        "platforms/discovery" => super::as_json(platforms::discover_platforms()),
        _ => return None,
    };

    Some(super::response(req, result))
}
