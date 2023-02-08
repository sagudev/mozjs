use libc::c_uint;
use mozjs::conversions::{ConversionResult, FromJSValConvertible};
use mozjs::jsapi::JSContext;
use mozjs::jsapi::JS_ClearPendingException;
use mozjs::jsapi::JS_IsExceptionPending;
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::wrappers::JS_ErrorFromException;
use mozjs::rust::wrappers::JS_GetPendingException;
use mozjs::rust::{HandleObject, HandleValue};
use std::slice::from_raw_parts;

/// A struct encapsulating information about a runtime script error.
pub struct ErrorInfo {
    /// The error message.
    pub message: String,
    /// The file name.
    pub filename: String,
    /// The line number.
    pub lineno: c_uint,
    /// The column number.
    pub column: c_uint,
}

impl ErrorInfo {
    unsafe fn from_native_error(object: HandleObject, cx: *mut JSContext) -> Option<ErrorInfo> {
        let report = JS_ErrorFromException(cx, object);
        if report.is_null() {
            return None;
        }

        let filename = {
            let filename = (*report)._base.filename as *const u8;
            if !filename.is_null() {
                let length = (0..).find(|idx| *filename.offset(*idx) == 0).unwrap();
                let filename = from_raw_parts(filename, length as usize);
                String::from_utf8_lossy(filename).into_owned()
            } else {
                "none".to_string()
            }
        };

        let lineno = (*report)._base.lineno;
        let column = (*report)._base.column;

        let message = {
            let message = (*report)._base.message_.data_ as *const u8;
            let length = (0..).find(|idx| *message.offset(*idx) == 0).unwrap();
            let message = from_raw_parts(message, length as usize);
            String::from_utf8_lossy(message).into_owned()
        };

        Some(ErrorInfo {
            filename,
            message,
            lineno,
            column,
        })
    }

    unsafe fn from_object(object: HandleObject, cx: *mut JSContext) -> Option<ErrorInfo> {
        if let Some(info) = ErrorInfo::from_native_error(object, cx) {
            return Some(info);
        }
        return None;
    }

    unsafe fn from_value(value: HandleValue, cx: *mut JSContext) -> ErrorInfo {
        if value.is_object() {
            rooted!(in(cx) let object = value.to_object());
            if let Some(info) = ErrorInfo::from_object(object.handle(), cx) {
                return info;
            }
        }

        if let Ok(ConversionResult::Success(string)) = String::from_jsval(cx, value, ()) {
            ErrorInfo {
                message: format!("uncaught exception: {}", string),
                filename: String::new(),
                lineno: 0,
                column: 0,
            }
        } else {
            panic!("uncaught exception: failed to stringify primitive");
        }
    }
}

// return true if exception was handled
pub unsafe fn handle_exception(cx: *mut JSContext) -> bool {
    if JS_IsExceptionPending(cx) {
        rooted!(in(cx) let mut exception = UndefinedValue());
        if JS_GetPendingException(cx, exception.handle_mut().into()) {
            JS_ClearPendingException(cx);
            let error_info = ErrorInfo::from_value(exception.handle(), cx);

            eprintln!(
                "Error at {}:{}:{} {}",
                error_info.filename, error_info.lineno, error_info.column, error_info.message
            );
            return true;
        }
    }
    false
}
