// Copyright (c) 2011 Google, Inc.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
//
// CityHash, by Geoff Pike and Jyrki Alakuijala
//
// This file provides CityHash64() and related functions.
//
// It's probably possible to create even faster hash functions by
// writing a program that systematically explores some of the space of
// possible hash functions, by using SIMD instructions, or by
// compromising on hash quality.

pub type Uint128 = (u64, u64);

fn fetch64(s: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(s[offset..offset + 8].try_into().unwrap())
}

fn fetch32(s: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(s[offset..offset + 4].try_into().unwrap())
}

// Some primes between 2^63 and 2^64 for various uses.
const K0: u64 = 0xc3a5c85c97cb3127;
const K1: u64 = 0xb492b66fbe98f273;
const K2: u64 = 0x9ae16a3b2f90404f;

// Bitwise right rotate.  Normally this will compile to a single
// instruction, especially if the shift is a manifest constant.
fn rotate(val: u64, shift: u32) -> u64 {
    // Avoid shifting by 64: doing so yields an undefined result.
    val.rotate_right(shift)
}

fn shift_mix(val: u64) -> u64 {
    val ^ (val >> 47)
}

// Hash 128 input bits down to 64 bits of output.
// This is intended to be a reasonably good hash function.
pub fn hash128_to_64(x: Uint128) -> u64 {
    // Murmur-inspired hashing.
    const K_MUL: u64 = 0x9ddfea08eb382d69;
    let mut a = (x.0 ^ x.1).wrapping_mul(K_MUL);
    a ^= a >> 47;
    let mut b = (x.1 ^ a).wrapping_mul(K_MUL);
    b ^= b >> 47;
    b = b.wrapping_mul(K_MUL);
    b
}

fn hash_len16(u: u64, v: u64) -> u64 {
    hash128_to_64((u, v))
}

fn hash_len16_mul(u: u64, v: u64, mul: u64) -> u64 {
    // Murmur-inspired hashing.
    let mut a = (u ^ v).wrapping_mul(mul);
    a ^= a >> 47;
    let mut b = (v ^ a).wrapping_mul(mul);
    b ^= b >> 47;
    b = b.wrapping_mul(mul);
    b
}

fn hash_len0to16(s: &[u8], len: usize) -> u64 {
    if len >= 8 {
        let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
        let a = fetch64(s, 0).wrapping_add(K2);
        let b = fetch64(s, len - 8);
        let c = rotate(b, 37).wrapping_mul(mul).wrapping_add(a);
        let d = rotate(a, 25).wrapping_add(b).wrapping_mul(mul);
        return hash_len16_mul(c, d, mul);
    }
    if len >= 4 {
        let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
        let a = u64::from(fetch32(s, 0));
        return hash_len16_mul(
            (len as u64).wrapping_add(a << 3),
            u64::from(fetch32(s, len - 4)),
            mul,
        );
    }
    if len > 0 {
        let a = s[0];
        let b = s[len >> 1];
        let c = s[len - 1];
        let y = u32::from(a).wrapping_add(u32::from(b) << 8);
        let z = (len as u32).wrapping_add(u32::from(c) << 2);
        return shift_mix(u64::from(y).wrapping_mul(K2) ^ u64::from(z).wrapping_mul(K0))
            .wrapping_mul(K2);
    }
    K2
}

// This probably works well for 16-byte strings as well, but it may be overkill
// in that case.
fn hash_len17to32(s: &[u8], len: usize) -> u64 {
    let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
    let a = fetch64(s, 0).wrapping_mul(K1);
    let b = fetch64(s, 8);
    let c = fetch64(s, len - 8).wrapping_mul(mul);
    let d = fetch64(s, len - 16).wrapping_mul(K2);
    hash_len16_mul(
        rotate(a.wrapping_add(b), 43)
            .wrapping_add(rotate(c, 30))
            .wrapping_add(d),
        a.wrapping_add(rotate(b.wrapping_add(K2), 18))
            .wrapping_add(c),
        mul,
    )
}

// Return a 16-byte hash for 48 bytes.  Quick and dirty.
// Callers do best to use "random-looking" values for a and b.
fn weak_hash_len32_with_seeds_values(
    w: u64,
    x: u64,
    y: u64,
    z: u64,
    mut a: u64,
    mut b: u64,
) -> (u64, u64) {
    a = a.wrapping_add(w);
    b = rotate(b.wrapping_add(a).wrapping_add(z), 21);
    let c = a;
    a = a.wrapping_add(x);
    a = a.wrapping_add(y);
    b = b.wrapping_add(rotate(a, 44));
    (a.wrapping_add(z), b.wrapping_add(c))
}

// Return a 16-byte hash for s[0] ... s[31], a, and b.  Quick and dirty.
fn weak_hash_len32_with_seeds(s: &[u8], offset: usize, a: u64, b: u64) -> (u64, u64) {
    weak_hash_len32_with_seeds_values(
        fetch64(s, offset),
        fetch64(s, offset + 8),
        fetch64(s, offset + 16),
        fetch64(s, offset + 24),
        a,
        b,
    )
}

// Return an 8-byte hash for 33 to 64 bytes.
fn hash_len33to64(s: &[u8], len: usize) -> u64 {
    let mul = K2.wrapping_add((len as u64).wrapping_mul(2));
    let mut a = fetch64(s, 0).wrapping_mul(K2);
    let mut b = fetch64(s, 8);
    let c = fetch64(s, len - 24);
    let d = fetch64(s, len - 32);
    let e = fetch64(s, 16).wrapping_mul(K2);
    let f = fetch64(s, 24).wrapping_mul(9);
    let g = fetch64(s, len - 8);
    let h = fetch64(s, len - 16).wrapping_mul(mul);
    let u =
        rotate(a.wrapping_add(g), 43).wrapping_add(rotate(b, 30).wrapping_add(c).wrapping_mul(9));
    let v = (a.wrapping_add(g) ^ d).wrapping_add(f).wrapping_add(1);
    let w = u
        .wrapping_add(v)
        .wrapping_mul(mul)
        .swap_bytes()
        .wrapping_add(h);
    let x = rotate(e.wrapping_add(f), 42).wrapping_add(c);
    let y = v
        .wrapping_add(w)
        .wrapping_mul(mul)
        .swap_bytes()
        .wrapping_add(g)
        .wrapping_mul(mul);
    let z = e.wrapping_add(f).wrapping_add(c);
    a = x
        .wrapping_add(z)
        .wrapping_mul(mul)
        .wrapping_add(y)
        .swap_bytes()
        .wrapping_add(b);
    b = shift_mix(
        z.wrapping_add(a)
            .wrapping_mul(mul)
            .wrapping_add(d)
            .wrapping_add(h),
    )
    .wrapping_mul(mul);
    b.wrapping_add(x)
}

// Hash function for a byte array.
pub fn city_hash64(s: &[u8]) -> u64 {
    let mut len = s.len();
    if len <= 32 {
        if len <= 16 {
            return hash_len0to16(s, len);
        } else {
            return hash_len17to32(s, len);
        }
    } else if len <= 64 {
        return hash_len33to64(s, len);
    }

    // For strings over 64 bytes we hash the end first, and then as we
    // loop we keep 56 bytes of state: v, w, x, y, and z.
    let mut x = fetch64(s, len - 40);
    let mut y = fetch64(s, len - 16).wrapping_add(fetch64(s, len - 56));
    let mut z = hash_len16(
        fetch64(s, len - 48).wrapping_add(len as u64),
        fetch64(s, len - 24),
    );
    let mut v = weak_hash_len32_with_seeds(s, len - 64, len as u64, z);
    let mut w = weak_hash_len32_with_seeds(s, len - 32, y.wrapping_add(K1), x);
    x = x.wrapping_mul(K1).wrapping_add(fetch64(s, 0));

    // Decrease len to the nearest multiple of 64, and operate on 64-byte chunks.
    len = (len - 1) & !63usize;
    let mut offset = 0;
    loop {
        x = rotate(
            x.wrapping_add(y)
                .wrapping_add(v.0)
                .wrapping_add(fetch64(s, offset + 8)),
            37,
        )
        .wrapping_mul(K1);
        y = rotate(
            y.wrapping_add(v.1).wrapping_add(fetch64(s, offset + 48)),
            42,
        )
        .wrapping_mul(K1);
        x ^= w.1;
        y = y.wrapping_add(v.0).wrapping_add(fetch64(s, offset + 40));
        z = rotate(z.wrapping_add(w.0), 33).wrapping_mul(K1);
        v = weak_hash_len32_with_seeds(s, offset, v.1.wrapping_mul(K1), x.wrapping_add(w.0));
        w = weak_hash_len32_with_seeds(
            s,
            offset + 32,
            z.wrapping_add(w.1),
            y.wrapping_add(fetch64(s, offset + 16)),
        );
        std::mem::swap(&mut z, &mut x);
        offset += 64;
        len -= 64;
        if len == 0 {
            break;
        }
    }
    hash_len16(
        hash_len16(v.0, w.0)
            .wrapping_add(shift_mix(y).wrapping_mul(K1))
            .wrapping_add(z),
        hash_len16(v.1, w.1).wrapping_add(x),
    )
}

// Hash function for a byte array.  For convenience, a 64-bit seed is also
// hashed into the result.
pub fn city_hash64_with_seed(s: &[u8], seed: u64) -> u64 {
    city_hash64_with_seeds(s, K2, seed)
}

// Hash function for a byte array.  For convenience, two seeds are also
// hashed into the result.
pub fn city_hash64_with_seeds(s: &[u8], seed0: u64, seed1: u64) -> u64 {
    hash_len16(city_hash64(s).wrapping_sub(seed0), seed1)
}

/// Computes a 64-bit hash over the specified block of data
/// @param data Block of data to compute hash over
/// @returns 64-bit hash value that was computed over the data block
pub fn compute_hash64(data: &[u8]) -> u64 {
    city_hash64(data)
}

/// Computes a 64-bit hash of a struct. In addition to being trivially copyable, it is also critical
/// that either the struct includes no padding, or that any padding is initialized to a known value
/// by memsetting the struct to 0 before filling it in.
pub fn compute_struct_hash64<T: Copy>(data: &T) -> u64 {
    let bytes =
        unsafe { std::slice::from_raw_parts((data as *const T).cast::<u8>(), size_of::<T>()) };
    compute_hash64(bytes)
}

/// Combines the seed parameter with the provided hash, producing a new unique hash
/// Implementation from: http://boost.sourceforge.net/doc/html/boost/hash_combine.html
pub fn hash_combine(seed: u64, hash: u64) -> u64 {
    seed ^ (hash
        .wrapping_add(0x9e3779b9)
        .wrapping_add(seed << 6)
        .wrapping_add(seed >> 2))
}
