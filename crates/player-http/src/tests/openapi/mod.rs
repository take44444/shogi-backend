use super::*;

#[path = "../support/openapi.rs"]
mod openapi_support;
use openapi_support::{
    Body, Request, UnusedRecords, Value, assert_local_refs_resolve, document, json,
};

mod runtime;
mod schema;

use openapi_support::{BodyExt, ServiceExt};
