// Copyright 2023 Greptime Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use std::collections::HashMap;
use std::time::Instant;

use axum::extract::{Json, Query, RawBody, State};
use common_catalog::consts::DEFAULT_CATALOG_NAME;
use common_error::ext::ErrorExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use session::context::QueryContext;

use crate::http::{ApiState, JsonResponse};

macro_rules! json_err {
    ($e: expr) => {{
        return Json(JsonResponse::with_error(
            format!("Invalid argument: {}", $e),
            common_error::status_code::StatusCode::InvalidArguments,
        ));
    }};

    ($msg: expr, $code: expr) => {{
        return Json(JsonResponse::with_error($msg.to_string(), $code));
    }};
}

macro_rules! unwrap_or_json_err {
    ($result: expr) => {
        match $result {
            Ok(result) => result,
            Err(e) => json_err!(e),
        }
    };
}

/// Handler to insert and compile script
#[axum_macros::debug_handler]
pub async fn scripts(
    State(state): State<ApiState>,
    Query(params): Query<ScriptQuery>,
    RawBody(body): RawBody,
) -> Json<JsonResponse> {
    if let Some(script_handler) = &state.script_handler {
        let catalog = params
            .catalog
            .unwrap_or_else(|| DEFAULT_CATALOG_NAME.to_string());
        let schema = params.db.as_ref();

        if schema.is_none() || schema.unwrap().is_empty() {
            json_err!("invalid schema")
        }

        let name = params.name.as_ref();

        if name.is_none() || name.unwrap().is_empty() {
            json_err!("invalid name");
        }

        let bytes = unwrap_or_json_err!(hyper::body::to_bytes(body).await);

        let script = unwrap_or_json_err!(String::from_utf8(bytes.to_vec()));

        // Safety: schema and name are already checked above.
        let query_ctx = QueryContext::with(&catalog, schema.unwrap());
        let body = match script_handler
            .insert_script(query_ctx, name.unwrap(), &script)
            .await
        {
            Ok(()) => JsonResponse::with_output(None),
            Err(e) => json_err!(format!("Insert script error: {e}"), e.status_code()),
        };

        Json(body)
    } else {
        json_err!("Script execution not supported, missing script handler");
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Default)]
pub struct ScriptQuery {
    pub catalog: Option<String>,
    pub db: Option<String>,
    pub name: Option<String>,
    #[serde(flatten)]
    pub params: HashMap<String, String>,
}

/// Handler to execute script
#[axum_macros::debug_handler]
pub async fn run_script(
    State(state): State<ApiState>,
    Query(params): Query<ScriptQuery>,
) -> Json<JsonResponse> {
    if let Some(script_handler) = &state.script_handler {
        let catalog = params
            .catalog
            .unwrap_or_else(|| DEFAULT_CATALOG_NAME.to_string());
        let start = Instant::now();
        let schema = params.db.as_ref();

        if schema.is_none() || schema.unwrap().is_empty() {
            json_err!("invalid schema")
        }

        let name = params.name.as_ref();

        if name.is_none() || name.unwrap().is_empty() {
            json_err!("invalid name");
        }

        // Safety: schema and name are already checked above.
        let query_ctx = QueryContext::with(&catalog, schema.unwrap());
        let output = script_handler
            .execute_script(query_ctx, name.unwrap(), params.params)
            .await;
        let resp = JsonResponse::from_output(vec![output]).await;

        Json(resp.with_execution_time(start.elapsed().as_millis()))
    } else {
        json_err!("Script execution not supported, missing script handler");
    }
}
