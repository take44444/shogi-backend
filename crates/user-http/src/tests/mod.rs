use super::*;

#[path = "support/openapi.rs"]
mod openapi_support;
use openapi_support::{
    Body, Request, Value, assert_local_refs_resolve, document, json, offline_router,
};

mod openapi_runtime;
mod openapi_schema;
mod profile_rename;

use openapi_support::{BodyExt, ServiceExt};
