use std::os::fd::RawFd;

use jni::objects::{JObjectArray, JString, JValue};

use super::android_jni::{call, call_bool, strings};
use super::{OpenMode, Storage};

/// Splits `path` into its parent and its name, because a document is created by its parent.
fn split_parent(path: &str) -> (&str, &str) {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => ("/", trimmed.trim_start_matches('/')),
        Some(index) => (&trimmed[..index], &trimmed[index + 1..]),
    }
}

/// Mode strings of `ParcelFileDescriptor.parseMode`, which differ from the ones of `fopen`.
fn descriptor_mode(mode: OpenMode) -> &'static str {
    match (mode.read, mode.write, mode.append, mode.truncate) {
        (true, false, _, _) => "r",
        (true, true, true, _) => "rwa",
        (true, true, _, true) => "rwt",
        (true, true, _, _) => "rw",
        (false, _, true, _) => "wa",
        (false, _, _, true) => "wt",
        _ => "w",
    }
}

/// Reaches the user directory of the Storage Access Framework and content URIs.
pub struct SafStorage;

impl Storage for SafStorage {
    fn exists(&self, path: &str) -> bool {
        call_bool("fileExists", "(Ljava/lang/String;)Z", &[path])
    }

    fn is_directory(&self, path: &str) -> bool {
        call_bool("isDirectory", "(Ljava/lang/String;)Z", &[path])
    }

    fn size(&self, path: &str) -> u64 {
        call(|env, class| {
            let path = env.new_string(path)?;
            env.call_static_method(class, "getSize", "(Ljava/lang/String;)J", &[
                JValue::Object(&path),
            ])?
            .j()
        })
        .map_or(0, |size| size.max(0) as u64)
    }

    fn create_dir(&self, path: &str) -> bool {
        let (parent, name) = split_parent(path);
        call_bool(
            "createDir",
            "(Ljava/lang/String;Ljava/lang/String;)Z",
            &[parent, name],
        )
    }

    fn remove_file(&self, path: &str) -> bool {
        call_bool("deleteDocument", "(Ljava/lang/String;)Z", &[path])
    }

    fn remove_dir(&self, path: &str) -> bool {
        call_bool("deleteDocument", "(Ljava/lang/String;)Z", &[path])
    }

    /// A document can only be renamed in place, so only the name of `to` is used.
    fn rename(&self, from: &str, to: &str) -> bool {
        let (_, name) = split_parent(to);
        call_bool(
            "renameFile",
            "(Ljava/lang/String;Ljava/lang/String;)Z",
            &[from, name],
        )
    }

    fn copy(&self, from: &str, to: &str) -> bool {
        let (parent, name) = split_parent(to);
        call_bool(
            "copyFile",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Z",
            &[from, parent, name],
        )
    }

    fn open(&self, path: &str, mode: OpenMode) -> Option<RawFd> {
        if mode.creates() && !self.exists(path) {
            let (parent, name) = split_parent(path);
            if !call_bool(
                "createFile",
                "(Ljava/lang/String;Ljava/lang/String;)Z",
                &[parent, name],
            ) {
                return None;
            }
        }
        let fd = call(|env, class| {
            let arguments = strings(env, &[path, descriptor_mode(mode)])?;
            let values: Vec<JValue> = arguments.iter().map(JValue::Object).collect();
            env.call_static_method(
                class,
                "openContentUri",
                "(Ljava/lang/String;Ljava/lang/String;)I",
                &values,
            )?
            .i()
        })?;
        (fd >= 0).then_some(fd)
    }

    fn list(&self, path: &str) -> Option<Vec<String>> {
        call(|env, class| {
            let path = env.new_string(path)?;
            let array = JObjectArray::from(
                env.call_static_method(
                    class,
                    "getFilesName",
                    "(Ljava/lang/String;)[Ljava/lang/String;",
                    &[JValue::Object(&path)],
                )?
                .l()?,
            );
            let length = env.get_array_length(&array)?;
            let mut names = Vec::with_capacity(length.max(0) as usize);
            for index in 0..length {
                let item = env.get_object_array_element(&array, index)?;
                if item.is_null() {
                    continue;
                }
                let item = JString::from(item);
                names.push(env.get_string(&item)?.into());
                env.delete_local_ref(item)?;
            }
            Ok(names)
        })
    }
}
