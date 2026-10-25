#ifndef EMU_JSON_H
#define EMU_JSON_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Opaque handle to one JSON value (RFC 8259).
 *
 * A handle is either owning or borrowed. Owning handles come from
 * emu_json_parse and the emu_json_new_* functions and are released with
 * emu_json_free. Borrowed handles come from emu_json_get, emu_json_array_at
 * and emu_json_object_value_at: they point into the owning value, stay valid
 * until that value is mutated or freed, and emu_json_free on them is a no-op,
 * so callers never have to track which kind they hold before freeing.
 *
 * Object keys keep their insertion order in every operation, including
 * emu_json_stringify. The format this replaces (nlohmann ordered_json) wrote
 * files such as the custom texture pack config in insertion order, and readers
 * compare against that output, so the order is part of this ABI.
 */
typedef struct EmuJson EmuJson;

enum EmuJsonType {
    EMU_JSON_NULL = 0,
    EMU_JSON_BOOL = 1,
    EMU_JSON_NUMBER = 2,
    EMU_JSON_STRING = 3,
    EMU_JSON_ARRAY = 4,
    EMU_JSON_OBJECT = 5,
};

/**
 * Parses `len` bytes of UTF-8 JSON text. Returns NULL on failure, in which
 * case emu_json_error describes the reason. Note that, unlike the nlohmann
 * parser this replaces in the custom texture manager, comments are not part
 * of RFC 8259 and are rejected.
 */
EmuJson* emu_json_parse(const char* text, size_t len);

/**
 * The reason the last emu_json_parse on this thread failed, or an empty
 * string when it did not. The pointer stays valid until the next emu_json_*
 * call on the same thread.
 */
const char* emu_json_error(void);

/** Releases an owning handle. NULL and borrowed handles are accepted. */
void emu_json_free(EmuJson* handle);

EmuJsonType emu_json_type(const EmuJson* handle);

/**
 * Resolves a path against a value and returns a borrowed handle to the
 * result, or NULL when any step is missing or has the wrong type.
 *
 * A path is dot-separated object keys, each optionally followed by one or
 * more [index] array subscripts, for example "rooms[0].name". A leading
 * [index] addresses an array root. The empty path returns the value itself.
 * Keys therefore cannot contain '.', '[' or ']' characters.
 */
EmuJson* emu_json_get(const EmuJson* handle, const char* path);

/** Borrowed handle to one array element, or NULL when out of range. */
EmuJson* emu_json_array_at(const EmuJson* handle, size_t index);

/** Borrowed handle to the value of the object entry at `index`, in insertion order. */
EmuJson* emu_json_object_value_at(const EmuJson* handle, size_t index);

/**
 * The string contents of a string value, or NULL for any other type. The
 * pointer is not NUL-terminated because the stored string is not; read
 * emu_json_string_len bytes. It stays valid until the value is mutated or
 * freed.
 */
const char* emu_json_string(const EmuJson* handle);
size_t emu_json_string_len(const EmuJson* handle);

/** The number as a double, an integer, or 0 when the value is not a number. */
double emu_json_number(const EmuJson* handle);
int64_t emu_json_i64(const EmuJson* handle);
uint64_t emu_json_u64(const EmuJson* handle);

/** 1 for true, 0 for false and for any non-boolean value. */
int emu_json_bool(const EmuJson* handle);

size_t emu_json_array_len(const EmuJson* handle);
size_t emu_json_object_len(const EmuJson* handle);

/**
 * The key of the object entry at `index`, in insertion order, or NULL when
 * out of range. Like emu_json_string, the result is not NUL-terminated and
 * `len` (when not NULL) receives its byte length.
 */
const char* emu_json_key_at(const EmuJson* handle, size_t index, size_t* len);

EmuJson* emu_json_new_object(void);
EmuJson* emu_json_new_array(void);
EmuJson* emu_json_new_string(const char* value);
EmuJson* emu_json_new_number_i64(int64_t value);
EmuJson* emu_json_new_number_u64(uint64_t value);
EmuJson* emu_json_new_number_f64(double value);
EmuJson* emu_json_new_bool(int value);
EmuJson* emu_json_new_null(void);

/**
 * Sets `key` on an object to `value`. emu_json_set and emu_json_push always
 * take ownership of `value`, including when they fail, so callers must not
 * use or free it afterwards either way. Setting an existing key replaces its
 * value and keeps its original position. Returns 1 on success, 0 when the
 * handle is not an object or an argument is invalid.
 */
int emu_json_set(EmuJson* object, const char* key, EmuJson* value);

/** Appends `value` to an array, under the same ownership rule as emu_json_set. */
int emu_json_push(EmuJson* array, EmuJson* value);

/**
 * Serializes a value. The result is a NUL-terminated string released with
 * emu_json_free_string, and `len` (when not NULL) receives its byte length
 * without the terminator. emu_json_stringify_indent writes one level of
 * indentation per nesting level as `indent` spaces, matching the indented
 * output previously produced for the custom texture pack config.
 */
char* emu_json_stringify(const EmuJson* handle, size_t* len);
char* emu_json_stringify_indent(const EmuJson* handle, size_t indent, size_t* len);
void emu_json_free_string(char* text);

#ifdef __cplusplus
}
#endif

#endif
