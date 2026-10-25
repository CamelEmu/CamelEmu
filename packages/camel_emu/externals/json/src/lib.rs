//! Standard JSON (RFC 8259) DOM backed by serde_json, exposed to C++ only
//! through the C ABI declared in `packages/camel_emu/include/emu_json.h`.
//!
//! Nothing specific to any caller crosses this boundary: type mapping and
//! missing-field handling for a particular format stay in the C++ shim of
//! that format. The unsafe surface is this module, which owns every handle
//! the C side holds.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

use serde_json::{Map, Value};

pub const EMU_JSON_NULL: u32 = 0;
pub const EMU_JSON_BOOL: u32 = 1;
pub const EMU_JSON_NUMBER: u32 = 2;
pub const EMU_JSON_STRING: u32 = 3;
pub const EMU_JSON_ARRAY: u32 = 4;
pub const EMU_JSON_OBJECT: u32 = 5;

/// One JSON value as seen from C. `owned` handles hold the only reference to
/// their value; borrowed ones point into an owning value and free as a no-op.
pub struct EmuJson {
    value: *mut Value,
    owned: bool,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").unwrap());
}

fn set_error(message: String) {
    LAST_ERROR.with(|error| {
        *error.borrow_mut() = CString::new(message)
            .unwrap_or_else(|_| CString::new("invalid error message").unwrap());
    });
}

fn clear_error() {
    LAST_ERROR.with(|error| {
        *error.borrow_mut() = CString::new("").unwrap();
    });
}

/// Runs an FFI body, turning a panic into `fallback` so it never unwinds
/// across the C boundary.
fn guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

fn owning(value: Value) -> *mut EmuJson {
    Box::into_raw(Box::new(EmuJson {
        value: Box::into_raw(Box::new(value)),
        owned: true,
    }))
}

fn borrowed(value: &Value) -> *mut EmuJson {
    Box::into_raw(Box::new(EmuJson {
        value: value as *const Value as *mut Value,
        owned: false,
    }))
}

/// # Safety
/// `handle` must be NULL or a handle returned by this crate that has not
/// been freed as an owning handle.
unsafe fn value_ref<'a>(handle: *const EmuJson) -> Option<&'a Value> {
    if handle.is_null() {
        return None;
    }
    unsafe {
        let handle = &*handle;
        if handle.value.is_null() {
            None
        } else {
            Some(&*handle.value)
        }
    }
}

/// # Safety
/// Same contract as [`value_ref`], and the caller must not hold another
/// reference to the same value while the mutable one is alive.
unsafe fn value_mut<'a>(handle: *mut EmuJson) -> Option<&'a mut Value> {
    if handle.is_null() {
        return None;
    }
    unsafe {
        let handle = &mut *handle;
        if handle.value.is_null() {
            None
        } else {
            Some(&mut *handle.value)
        }
    }
}

/// # Safety
/// Same contract as [`value_ref`].
unsafe fn free_handle(handle: *mut EmuJson) {
    if handle.is_null() {
        return;
    }
    unsafe {
        let handle = Box::from_raw(handle);
        if handle.owned && !handle.value.is_null() {
            drop(Box::from_raw(handle.value));
        }
    }
}

fn resolve_path<'a>(mut current: &'a Value, path: &str) -> Option<&'a Value> {
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'.' {
            i += 1;
            continue;
        }
        if bytes[i] == b'[' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == start || end >= bytes.len() || bytes[end] != b']' {
                return None;
            }
            let index: usize = path[start..end].parse().ok()?;
            current = current.as_array()?.get(index)?;
            i = end + 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i] != b'.' && bytes[i] != b'[' {
            i += 1;
        }
        current = current.as_object()?.get(&path[start..i])?;
    }
    Some(current)
}

fn type_of(value: &Value) -> u32 {
    match value {
        Value::Null => EMU_JSON_NULL,
        Value::Bool(_) => EMU_JSON_BOOL,
        Value::Number(_) => EMU_JSON_NUMBER,
        Value::String(_) => EMU_JSON_STRING,
        Value::Array(_) => EMU_JSON_ARRAY,
        Value::Object(_) => EMU_JSON_OBJECT,
    }
}

/// # Safety
/// `text` must be readable for `len` bytes or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_parse(text: *const c_char, len: usize) -> *mut EmuJson {
    guard(ptr::null_mut(), || {
        clear_error();
        if text.is_null() {
            set_error("cannot parse a null text".to_string());
            return ptr::null_mut();
        }
        let bytes = unsafe { std::slice::from_raw_parts(text as *const u8, len) };
        match serde_json::from_slice::<Value>(bytes) {
            Ok(value) => owning(value),
            Err(error) => {
                set_error(error.to_string());
                ptr::null_mut()
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_error() -> *const c_char {
    guard(ptr::null(), || {
        LAST_ERROR.with(|error| error.borrow().as_ptr())
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_free(handle: *mut EmuJson) {
    guard((), || unsafe { free_handle(handle) });
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_type(handle: *const EmuJson) -> u32 {
    guard(EMU_JSON_NULL, || match unsafe { value_ref(handle) } {
        Some(value) => type_of(value),
        None => EMU_JSON_NULL,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`] and `path` must be a
/// NUL-terminated string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_get(handle: *const EmuJson, path: *const c_char) -> *mut EmuJson {
    guard(ptr::null_mut(), || {
        let Some(root) = (unsafe { value_ref(handle) }) else {
            return ptr::null_mut();
        };
        if path.is_null() {
            return ptr::null_mut();
        }
        let Ok(path) = unsafe { CStr::from_ptr(path) }.to_str() else {
            return ptr::null_mut();
        };
        match resolve_path(root, path) {
            Some(value) => borrowed(value),
            None => ptr::null_mut(),
        }
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_array_at(handle: *const EmuJson, index: usize) -> *mut EmuJson {
    guard(ptr::null_mut(), || {
        let Some(value) = (unsafe { value_ref(handle) }) else {
            return ptr::null_mut();
        };
        match value.as_array().and_then(|array| array.get(index)) {
            Some(value) => borrowed(value),
            None => ptr::null_mut(),
        }
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_object_value_at(
    handle: *const EmuJson,
    index: usize,
) -> *mut EmuJson {
    guard(ptr::null_mut(), || {
        let Some(value) = (unsafe { value_ref(handle) }) else {
            return ptr::null_mut();
        };
        match value
            .as_object()
            .and_then(|object| object.values().nth(index))
        {
            Some(value) => borrowed(value),
            None => ptr::null_mut(),
        }
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_string(handle: *const EmuJson) -> *const c_char {
    guard(ptr::null(), || match unsafe { value_ref(handle) } {
        Some(Value::String(text)) => text.as_ptr() as *const c_char,
        _ => ptr::null(),
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_string_len(handle: *const EmuJson) -> usize {
    guard(0, || match unsafe { value_ref(handle) } {
        Some(Value::String(text)) => text.len(),
        _ => 0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_number(handle: *const EmuJson) -> f64 {
    guard(0.0, || match unsafe { value_ref(handle) } {
        Some(value) => value.as_f64().unwrap_or(0.0),
        None => 0.0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_i64(handle: *const EmuJson) -> i64 {
    guard(0, || match unsafe { value_ref(handle) } {
        Some(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
            .unwrap_or(0),
        None => 0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_u64(handle: *const EmuJson) -> u64 {
    guard(0, || match unsafe { value_ref(handle) } {
        Some(value) => value
            .as_u64()
            .or_else(|| value.as_i64().and_then(|v| u64::try_from(v).ok()))
            .unwrap_or(0),
        None => 0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_bool(handle: *const EmuJson) -> libc::c_int {
    guard(0, || match unsafe { value_ref(handle) } {
        Some(value) => value.as_bool().unwrap_or(false) as libc::c_int,
        None => 0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_array_len(handle: *const EmuJson) -> usize {
    guard(0, || match unsafe { value_ref(handle) } {
        Some(value) => value.as_array().map_or(0, Vec::len),
        None => 0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_object_len(handle: *const EmuJson) -> usize {
    guard(0, || match unsafe { value_ref(handle) } {
        Some(value) => value.as_object().map_or(0, Map::len),
        None => 0,
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`] and `len` must be
/// writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_key_at(
    handle: *const EmuJson,
    index: usize,
    len: *mut usize,
) -> *const c_char {
    guard(ptr::null(), || {
        let Some(value) = (unsafe { value_ref(handle) }) else {
            return ptr::null();
        };
        let Some(key) = value
            .as_object()
            .and_then(|object| object.keys().nth(index))
        else {
            return ptr::null();
        };
        if !len.is_null() {
            unsafe { *len = key.len() };
        }
        key.as_ptr() as *const c_char
    })
}

fn new_value(value: Value) -> *mut EmuJson {
    guard(ptr::null_mut(), || owning(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_object() -> *mut EmuJson {
    new_value(Value::Object(Map::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_array() -> *mut EmuJson {
    new_value(Value::Array(Vec::new()))
}

/// # Safety
/// `value` must be a NUL-terminated string or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_new_string(value: *const c_char) -> *mut EmuJson {
    guard(ptr::null_mut(), || {
        if value.is_null() {
            return ptr::null_mut();
        }
        match unsafe { CStr::from_ptr(value) }.to_str() {
            Ok(text) => owning(Value::String(text.to_string())),
            Err(_) => ptr::null_mut(),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_number_i64(value: i64) -> *mut EmuJson {
    new_value(Value::from(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_number_u64(value: u64) -> *mut EmuJson {
    new_value(Value::from(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_number_f64(value: f64) -> *mut EmuJson {
    new_value(serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number))
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_bool(value: libc::c_int) -> *mut EmuJson {
    new_value(Value::Bool(value != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_json_new_null() -> *mut EmuJson {
    new_value(Value::Null)
}

/// Consumes a handle passed to emu_json_set or emu_json_push. Only an owning
/// handle carries a value that can move into another value, so a borrowed
/// handle is released without one.
///
/// # Safety
/// `handle` must follow the contract of [`value_ref`].
unsafe fn take_owned_value(handle: *mut EmuJson) -> Option<Value> {
    if handle.is_null() {
        return None;
    }
    unsafe {
        let handle = Box::from_raw(handle);
        if !handle.owned || handle.value.is_null() {
            if handle.owned && !handle.value.is_null() {
                drop(Box::from_raw(handle.value));
            }
            return None;
        }
        Some(*Box::from_raw(handle.value))
    }
}

/// # Safety
/// `object` and `value` must follow the contract of [`value_ref`], `key`
/// must be a NUL-terminated string or NULL. `value` is consumed in all
/// cases, as the header declares.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_set(
    object: *mut EmuJson,
    key: *const c_char,
    value: *mut EmuJson,
) -> libc::c_int {
    guard(0, || {
        // The value is consumed even on failure, so move it out of its
        // handle before any early return below. A borrowed value cannot be
        // moved: it still belongs to its parent and is only released.
        let Some(inner) = (unsafe { take_owned_value(value) }) else {
            return 0;
        };
        if key.is_null() {
            return 0;
        }
        let Ok(key) = unsafe { CStr::from_ptr(key) }.to_str() else {
            return 0;
        };
        let key = key.to_string();
        let Some(target) = (unsafe { value_mut(object) }) else {
            return 0;
        };
        let Some(map) = target.as_object_mut() else {
            return 0;
        };
        map.insert(key, inner);
        1
    })
}

/// # Safety
/// `array` and `value` must follow the contract of [`value_ref`]. `value`
/// is consumed in all cases, as the header declares.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_push(array: *mut EmuJson, value: *mut EmuJson) -> libc::c_int {
    guard(0, || {
        // Same consumption rule as emu_json_set.
        let Some(inner) = (unsafe { take_owned_value(value) }) else {
            return 0;
        };
        let Some(target) = (unsafe { value_mut(array) }) else {
            return 0;
        };
        let Some(items) = target.as_array_mut() else {
            return 0;
        };
        items.push(inner);
        1
    })
}

/// Writes a value with one indentation level per nesting level. Scalars go
/// through serde_json so their spelling (string escapes, number formatting)
/// is identical to the compact form; only the whitespace is added here.
fn write_indented(value: &Value, indent: usize, level: usize, out: &mut String) {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(indent * (level + 1)));
                write_indented(item, indent, level + 1, out);
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent * level));
            out.push(']');
        }
        Value::Object(entries) => {
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (index, (key, item)) in entries.iter().enumerate() {
                if index > 0 {
                    out.push_str(",\n");
                }
                out.push_str(&" ".repeat(indent * (level + 1)));
                if let Ok(key) = serde_json::to_string(key) {
                    out.push_str(&key);
                }
                out.push_str(": ");
                write_indented(item, indent, level + 1, out);
            }
            out.push('\n');
            out.push_str(&" ".repeat(indent * level));
            out.push('}');
        }
        _ => {
            if let Ok(text) = serde_json::to_string(value) {
                out.push_str(&text);
            }
        }
    }
}

fn stringify(handle: *const EmuJson, indent: usize) -> *mut c_char {
    let Some(value) = (unsafe { value_ref(handle) }) else {
        return ptr::null_mut();
    };
    let text = if indent == 0 {
        serde_json::to_string(value).ok()
    } else {
        let mut out = String::new();
        write_indented(value, indent, 0, &mut out);
        Some(out)
    };
    match text.and_then(|text| CString::new(text).ok()) {
        Some(cstring) => cstring.into_raw(),
        None => ptr::null_mut(),
    }
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`] and `len` must be
/// writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_stringify(
    handle: *const EmuJson,
    len: *mut usize,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let text = stringify(handle, 0);
        if !text.is_null() && !len.is_null() {
            unsafe { *len = CStr::from_ptr(text).to_bytes().len() };
        }
        text
    })
}

/// # Safety
/// `handle` must follow the contract of [`value_ref`] and `len` must be
/// writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_stringify_indent(
    handle: *const EmuJson,
    indent: usize,
    len: *mut usize,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        let text = stringify(handle, indent);
        if !text.is_null() && !len.is_null() {
            unsafe { *len = CStr::from_ptr(text).to_bytes().len() };
        }
        text
    })
}

/// # Safety
/// `text` must be a string returned by emu_json_stringify or
/// emu_json_stringify_indent, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_json_free_string(text: *mut c_char) {
    guard((), || {
        if !text.is_null() {
            unsafe { drop(CString::from_raw(text)) };
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cstring(text: &str) -> CString {
        CString::new(text).unwrap()
    }

    fn stringify_of(handle: *const EmuJson) -> String {
        unsafe {
            let raw = emu_json_stringify(handle, ptr::null_mut());
            assert!(!raw.is_null());
            let text = CStr::from_ptr(raw).to_str().unwrap().to_string();
            emu_json_free_string(raw);
            text
        }
    }

    fn string_of(handle: *const EmuJson) -> String {
        unsafe {
            let ptr = emu_json_string(handle);
            assert!(!ptr.is_null());
            let len = emu_json_string_len(handle);
            std::str::from_utf8(std::slice::from_raw_parts(ptr as *const u8, len))
                .unwrap()
                .to_string()
        }
    }

    unsafe fn set_str(object: *mut EmuJson, key: &str, value: &str) {
        let key = cstring(key);
        let value = unsafe { emu_json_new_string(cstring(value).as_ptr()) };
        assert_eq!(unsafe { emu_json_set(object, key.as_ptr(), value) }, 1);
    }

    unsafe fn set_bool(object: *mut EmuJson, key: &str, value: bool) {
        let key = cstring(key);
        let value = emu_json_new_bool(value as libc::c_int);
        assert_eq!(unsafe { emu_json_set(object, key.as_ptr(), value) }, 1);
    }

    #[test]
    fn stringify_keeps_insertion_order() {
        unsafe {
            let root = emu_json_new_object();
            set_str(root, "b", "1");
            let key = cstring("a");
            assert_eq!(
                emu_json_set(root, key.as_ptr(), emu_json_new_number_i64(2)),
                1
            );
            set_str(root, "c", "3");
            assert_eq!(stringify_of(root), "{\"b\":\"1\",\"a\":2,\"c\":\"3\"}");
            emu_json_free(root);
        }
    }

    #[test]
    fn replacing_a_key_keeps_its_position() {
        unsafe {
            let root = emu_json_new_object();
            set_str(root, "a", "1");
            set_str(root, "b", "2");
            set_str(root, "a", "3");
            assert_eq!(stringify_of(root), "{\"a\":\"3\",\"b\":\"2\"}");
            emu_json_free(root);
        }
    }

    #[test]
    fn parse_and_get_by_path() {
        unsafe {
            let text = br#"{"rooms":[{"name":"room","id":18446744073709551615}],"ok":true}"#;
            let root = emu_json_parse(text.as_ptr() as *const c_char, text.len());
            assert!(!root.is_null());
            let name = emu_json_get(root, cstring("rooms[0].name").as_ptr());
            assert!(!name.is_null());
            assert_eq!(emu_json_type(name), EMU_JSON_STRING);
            assert_eq!(string_of(name), "room");
            emu_json_free(name);
            let id = emu_json_get(root, cstring("rooms[0].id").as_ptr());
            assert_eq!(emu_json_u64(id), u64::MAX);
            emu_json_free(id);
            let ok = emu_json_get(root, cstring("ok").as_ptr());
            assert_eq!(emu_json_bool(ok), 1);
            emu_json_free(ok);
            assert!(emu_json_get(root, cstring("rooms[1]").as_ptr()).is_null());
            assert!(emu_json_get(root, cstring("missing.key").as_ptr()).is_null());
            emu_json_free(root);
        }
    }

    #[test]
    fn parse_failure_reports_an_error() {
        unsafe {
            let text = b"{not json";
            let root = emu_json_parse(text.as_ptr() as *const c_char, text.len());
            assert!(root.is_null());
            let error = emu_json_error();
            assert!(!error.is_null());
            assert!(!CStr::from_ptr(error).to_str().unwrap().is_empty());
        }
    }

    #[test]
    fn comments_are_rejected() {
        unsafe {
            let text = b"{\"a\": 1} // comment";
            assert!(emu_json_parse(text.as_ptr() as *const c_char, text.len()).is_null());
            let text = b"{/* comment */\"a\": 1}";
            assert!(emu_json_parse(text.as_ptr() as *const c_char, text.len()).is_null());
        }
    }

    #[test]
    fn arrays_push_len_and_at() {
        unsafe {
            let array = emu_json_new_array();
            assert_eq!(emu_json_push(array, emu_json_new_number_u64(7)), 1);
            assert_eq!(emu_json_push(array, emu_json_new_number_f64(1.5)), 1);
            assert_eq!(emu_json_array_len(array), 2);
            let first = emu_json_array_at(array, 0);
            assert_eq!(emu_json_u64(first), 7);
            emu_json_free(first);
            let second = emu_json_array_at(array, 1);
            assert!((emu_json_number(second) - 1.5).abs() < f64::EPSILON);
            emu_json_free(second);
            assert!(emu_json_array_at(array, 2).is_null());
            assert_eq!(stringify_of(array), "[7,1.5]");
            emu_json_free(array);
        }
    }

    #[test]
    fn object_keys_iterate_in_insertion_order() {
        unsafe {
            let root = emu_json_new_object();
            set_str(root, "z", "1");
            set_str(root, "m", "2");
            assert_eq!(emu_json_object_len(root), 2);
            for (index, expected) in ["z", "m"].iter().enumerate() {
                let mut len = 0;
                let key = emu_json_key_at(root, index, &mut len);
                let key =
                    std::str::from_utf8(std::slice::from_raw_parts(key as *const u8, len)).unwrap();
                assert_eq!(key, *expected);
                let value = emu_json_object_value_at(root, index);
                assert_eq!(emu_json_type(value), EMU_JSON_STRING);
                emu_json_free(value);
            }
            emu_json_free(root);
        }
    }

    #[test]
    fn borrowed_handles_can_be_mutated_through_the_parent() {
        unsafe {
            let root = emu_json_new_object();
            let options = emu_json_new_object();
            let key = cstring("options");
            assert_eq!(emu_json_set(root, key.as_ptr(), options), 1);
            let borrowed = emu_json_get(root, cstring("options").as_ptr());
            assert!(!borrowed.is_null());
            set_bool(borrowed, "skip_mipmap", false);
            emu_json_free(borrowed);
            assert_eq!(stringify_of(root), "{\"options\":{\"skip_mipmap\":false}}");
            emu_json_free(root);
        }
    }

    #[test]
    fn indented_stringify_matches_the_pack_config_layout() {
        unsafe {
            let root = emu_json_new_object();
            set_str(root, "author", "citra");
            set_str(root, "version", "1.0.0");
            set_str(root, "description", "A graphics pack");
            let options = emu_json_new_object();
            set_bool(options, "skip_mipmap", false);
            set_bool(options, "flip_png_files", true);
            set_bool(options, "use_new_hash", true);
            let key = cstring("options");
            assert_eq!(emu_json_set(root, key.as_ptr(), options), 1);
            let raw = emu_json_stringify_indent(root, 4, ptr::null_mut());
            let text = CStr::from_ptr(raw).to_str().unwrap().to_string();
            emu_json_free_string(raw);
            let expected = "{\n    \"author\": \"citra\",\n    \"version\": \"1.0.0\",\n    \"description\": \"A graphics pack\",\n    \"options\": {\n        \"skip_mipmap\": false,\n        \"flip_png_files\": true,\n        \"use_new_hash\": true\n    }\n}";
            assert_eq!(text, expected);
            emu_json_free(root);
        }
    }

    #[test]
    fn set_on_a_non_object_consumes_the_value_and_fails() {
        unsafe {
            let array = emu_json_new_array();
            let key = cstring("key");
            assert_eq!(
                emu_json_set(
                    array,
                    key.as_ptr(),
                    emu_json_new_string(cstring("v").as_ptr())
                ),
                0
            );
            emu_json_free(array);
        }
    }

    #[test]
    fn null_handles_are_safe() {
        unsafe {
            assert_eq!(emu_json_type(ptr::null()), EMU_JSON_NULL);
            assert_eq!(emu_json_array_len(ptr::null()), 0);
            assert!(emu_json_string(ptr::null()).is_null());
            emu_json_free(ptr::null_mut());
            emu_json_free_string(ptr::null_mut());
        }
    }
}
