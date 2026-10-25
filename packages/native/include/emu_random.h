#ifndef EMU_RANDOM_H
#define EMU_RANDOM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Fills out[0, len) with cryptographically secure random bytes from the
 * operating system.
 *
 * Does nothing when out is NULL or len is 0. The call cannot fail: if the
 * system cannot provide randomness the process aborts rather than return bytes
 * that are not random. The generator cannot be seeded.
 */
void emu_random_fill(uint8_t* out, size_t len);

#ifdef __cplusplus
}
#endif

#endif
