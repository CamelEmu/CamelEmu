use std::ffi::c_void;
use std::sync::OnceLock;

use jni::errors::Result as JniResult;
use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};

/// The Storage Access Framework and the directories of the application can only be reached from
/// Java, and `NativeLibrary` already keeps the document tree that resolves the paths of the core,
/// so the Android backends call into it.
const NATIVE_LIBRARY: &str = "com.karasu256.azahar_reloaded.lib.azahar_for_flutter.NativeLibrary";

unsafe extern "C" {
    fn azahar_host_java_vm() -> *mut c_void;
    fn azahar_host_class_loader() -> *mut c_void;
}

struct Java {
    vm: JavaVM,
    class: GlobalRef,
}

static JAVA: OnceLock<Java> = OnceLock::new();

/// Created on first use instead of at load time, because the Java VM is only known after
/// `JNI_OnLoad` ran.
fn java() -> Option<&'static Java> {
    if let Some(java) = JAVA.get() {
        return Some(java);
    }
    let vm = unsafe { azahar_host_java_vm() };
    let loader = unsafe { azahar_host_class_loader() };
    if vm.is_null() || loader.is_null() {
        return None;
    }
    let vm = unsafe { JavaVM::from_raw(vm.cast()) }.ok()?;
    let class = {
        let mut env = vm.attach_current_thread_permanently().ok()?;
        let loader = unsafe { JObject::from_raw(loader.cast()) };
        let name = env.new_string(NATIVE_LIBRARY).ok()?;
        let class = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&name)],
            )
            .and_then(|value| value.l());
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        env.new_global_ref(class.ok()?).ok()?
    };
    let _ = JAVA.set(Java { vm, class });
    JAVA.get()
}

/// Runs `f` in a local frame, because the threads of the core stay attached and would otherwise
/// keep every local reference alive.
pub(super) fn call<T>(
    f: impl FnOnce(&mut JNIEnv, &JClass<'static>) -> JniResult<T>,
) -> Option<T> {
    let java = java()?;
    let mut env = java.vm.attach_current_thread_permanently().ok()?;
    let class = <&JClass>::from(java.class.as_obj());
    let result = env.with_local_frame(16, |env| f(env, class));
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    result.ok()
}

pub(super) fn strings<'local>(
    env: &mut JNIEnv<'local>,
    values: &[&str],
) -> JniResult<Vec<JObject<'local>>> {
    values
        .iter()
        .map(|value| env.new_string(value).map(JObject::from))
        .collect()
}

pub(super) fn call_bool(name: &str, signature: &str, arguments: &[&str]) -> bool {
    call(|env, class| {
        let arguments = strings(env, arguments)?;
        let values: Vec<JValue> = arguments.iter().map(JValue::Object).collect();
        env.call_static_method(class, name, signature, &values)?.z()
    })
    .unwrap_or(false)
}
