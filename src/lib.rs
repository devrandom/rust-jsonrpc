// Rust JSON-RPC Library
// Written in 2015 by
//   Andrew Poelstra <apoelstra@wpsoftware.net>
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

//! # Rust JSON-RPC Library
//!
//! Rust support for the JSON-RPC 2.0 protocol.
//!

// Coding conventions
#![deny(non_upper_case_globals)]
#![deny(non_camel_case_types)]
#![deny(non_snake_case)]
#![deny(unused_mut)]
#![warn(missing_docs)]

extern crate serde;
#[macro_use]
extern crate serde_derive;
pub extern crate serde_json;

#[cfg(feature = "base64-compat")]
pub extern crate base64;

pub mod client;
pub mod error;
mod util;

#[cfg(feature = "simple_http")]
pub mod simple_http;

#[cfg(feature = "simple_tcp")]
pub mod simple_tcp;

#[cfg(all(feature = "simple_uds", not(windows)))]
pub mod simple_uds;

// Re-export error type
pub use error::Error;
pub use client::{Client, Transport};

use serde_json::value::RawValue;

/// Shorthand method to convert an argument into a [Box<serde_json::value::RawValue>].
/// Since serializers rarely fail, it's probably easier to use [arg] instead.
pub fn try_arg<T: serde::Serialize>(arg: T) -> Result<Box<RawValue>, serde_json::Error> {
    RawValue::from_string(serde_json::to_string(&arg)?)
}

/// Shorthand method to convert an argument into a [Box<serde_json::value::RawValue>].
///
/// This conversion should not fail, so to avoid returning a [Result],
/// in case of an error, the error is serialized as the return value.
pub fn arg<T: serde::Serialize>(arg: T) -> Box<RawValue> {
    match try_arg(arg) {
        Ok(v) => v,
        Err(e) => RawValue::from_string(format!("<<ERROR SERIALIZING ARGUMENT: {}>>", e))
            .unwrap_or(RawValue::from_string("<<ERROR SERIALIZING ARGUMENT>>".to_owned()).unwrap()),
    }
}

#[derive(Debug, Clone, Serialize)]
/// A JSONRPC request object
pub struct Request<'a> {
    /// The name of the RPC call
    pub method: &'a str,
    /// Parameters to the RPC call
    pub params: &'a [Box<RawValue>],
    /// Identifier for this Request, which should appear in the response
    pub id: serde_json::Value,
    /// jsonrpc field, MUST be "2.0"
    pub jsonrpc: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
/// A JSONRPC response object
pub struct Response {
    /// A result if there is one, or null
    pub result: Option<Box<RawValue>>,
    /// An error if there is one, or null
    pub error: Option<error::RpcError>,
    /// Identifier for this Request, which should match that of the request
    pub id: serde_json::Value,
    /// jsonrpc field, MUST be "2.0"
    pub jsonrpc: Option<String>,
}

impl Response {
    /// Extract the result from a response
    pub fn result<T: for<'a> serde::de::Deserialize<'a>>(&self) -> Result<T, Error> {
        if let Some(ref e) = self.error {
            return Err(Error::Rpc(e.clone()));
        }

        if let Some(ref res) = self.result {
            serde_json::from_str(res.get()).map_err(Error::Json)
        } else {
            serde_json::from_value(serde_json::Value::Null).map_err(Error::Json)
        }
    }

    /// Return the RPC error, if there was one, but do not check the result
    pub fn check_error(self) -> Result<(), Error> {
        if let Some(e) = self.error {
            Err(Error::Rpc(e))
        } else {
            Ok(())
        }
    }

    /// Returns whether or not the `result` field is empty
    pub fn is_none(&self) -> bool {
        self.result.is_none()
    }
}

#[cfg(test)]
mod tests {

    use super::Response;
    use serde_json;
    use serde_json::value::RawValue;

    #[test]
    fn response_is_none() {
        let joanna = Response {
            result: Some(RawValue::from_string(serde_json::to_string(&true).unwrap()).unwrap()),
            error: None,
            id: From::from(81),
            jsonrpc: Some(String::from("2.0")),
        };

        let bill = Response {
            result: None,
            error: None,
            id: From::from(66),
            jsonrpc: Some(String::from("2.0")),
        };

        assert!(!joanna.is_none());
        assert!(bill.is_none());
    }

    #[test]
    fn response_extract() {
        let obj = vec!["Mary", "had", "a", "little", "lamb"];
        let response = Response {
            result: Some(RawValue::from_string(serde_json::to_string(&obj).unwrap()).unwrap()),
            error: None,
            id: serde_json::Value::Null,
            jsonrpc: Some(String::from("2.0")),
        };
        let recovered1: Vec<String> = response.result().unwrap();
        assert!(response.clone().check_error().is_ok());
        let recovered2: Vec<String> = response.result().unwrap();
        assert_eq!(obj, recovered1);
        assert_eq!(obj, recovered2);
    }

    #[test]
    fn null_result() {
        let s = r#"{"result":null,"error":null,"id":"test"}"#;
        let response: Response = serde_json::from_str(&s).unwrap();
        let recovered1: Result<(), _> = response.result();
        let recovered2: Result<(), _> = response.clone().result();
        assert!(recovered1.is_ok());
        assert!(recovered2.is_ok());

        let recovered1: Result<String, _> = response.result();
        let recovered2: Result<String, _> = response.clone().result();
        assert!(recovered1.is_err());
        assert!(recovered2.is_err());
    }

    #[test]
    fn batch_response() {
        // from the jsonrpc.org spec example
        let s = r#"[
            {"jsonrpc": "2.0", "result": 7, "id": "1"},
            {"jsonrpc": "2.0", "result": 19, "id": "2"},
            {"jsonrpc": "2.0", "error": {"code": -32600, "message": "Invalid Request"}, "id": null},
            {"jsonrpc": "2.0", "error": {"code": -32601, "message": "Method not found"}, "id": "5"},
            {"jsonrpc": "2.0", "result": ["hello", 5], "id": "9"}
        ]"#;
        let batch_response: Vec<Response> = serde_json::from_str(&s).unwrap();
        assert_eq!(batch_response.len(), 5);
    }

    #[test]
    fn test_arg() {
        macro_rules! test_arg {
            ($val:expr, $t:ty) => {{
                let val1: $t = $val;
                let arg = super::arg(val1.clone());
                let val2: $t = serde_json::from_str(arg.get()).expect(stringify!($val));
                assert_eq!(val1, val2, "failed test for {}", stringify!($val));
            }}
        }

        test_arg!(true, bool);
        test_arg!(42, u8);
        test_arg!(42, usize);
        test_arg!(42, isize);
        test_arg!(vec![42, 35], Vec<u8>);
        test_arg!(String::from("test"), String);

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        struct Test {
            v: String,
        }
        test_arg!(Test { v: String::from("test"), }, Test);
    }
}
