use sha2::{Digest, Sha256};

const FIELD_BITS: usize = 233;
pub const COORDINATE_SIZE: usize = 0x1E;
pub const PUBLIC_KEY_SIZE: usize = COORDINATE_SIZE * 2;
pub const SIGNATURE_SIZE: usize = COORDINATE_SIZE * 2;

type Words = [u64; 4];
type Point = Option<(Words, Words)>;

const ZERO: Words = [0; 4];
const ONE: Words = [1, 0, 0, 0];

fn from_be(bytes: &[u8]) -> Option<Words> {
    if bytes.len() > 32 {
        return None;
    }
    let mut padded = [0u8; 32];
    padded[32 - bytes.len()..].copy_from_slice(bytes);
    let mut words = ZERO;
    for (index, chunk) in padded.rchunks(8).enumerate() {
        words[index] = u64::from_be_bytes(chunk.try_into().ok()?);
    }
    Some(words)
}

fn to_be(words: &Words) -> [u8; COORDINATE_SIZE] {
    let mut full = [0u8; 32];
    for (index, chunk) in full.rchunks_mut(8).enumerate() {
        chunk.copy_from_slice(&words[index].to_be_bytes());
    }
    full[32 - COORDINATE_SIZE..].try_into().unwrap()
}

fn constant(hex: &str) -> Words {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect();
    from_be(&bytes).unwrap()
}

fn curve_b() -> Words {
    constant("0066647ede6c332c7f8c0923bb58213b333b20e9ce4281fe115f7d8f90ad")
}

fn generator() -> (Words, Words) {
    (
        constant("00fac9dfcbac8313bb2139f1bb755fef65bc391f8b36f8f8eb7371fd558b"),
        constant("01006a08a41903350678e58528bebf8a0beff867a7ca36716f7e01f81052"),
    )
}

fn order() -> Words {
    constant("01000000000000000000000000000013e974e72f8a6922031d2603cfe0d7")
}

fn bit(words: &Words, index: usize) -> bool {
    words[index / 64] >> (index % 64) & 1 == 1
}

fn degree(words: &Words) -> Option<usize> {
    (0..4)
        .rev()
        .find(|&index| words[index] != 0)
        .map(|index| index * 64 + 63 - words[index].leading_zeros() as usize)
}

fn xor(a: &Words, b: &Words) -> Words {
    [a[0] ^ b[0], a[1] ^ b[1], a[2] ^ b[2], a[3] ^ b[3]]
}

fn shift_left(words: &Words, count: usize) -> Words {
    let (whole, part) = (count / 64, count % 64);
    let mut result = ZERO;
    for index in whole..4 {
        result[index] = words[index - whole] << part;
        if part != 0 && index > whole {
            result[index] |= words[index - whole - 1] >> (64 - part);
        }
    }
    result
}

fn shift_right(words: &Words, count: usize) -> Words {
    let (whole, part) = (count / 64, count % 64);
    let mut result = ZERO;
    for index in 0..4 - whole {
        result[index] = words[index + whole] >> part;
        if part != 0 && index + whole + 1 < 4 {
            result[index] |= words[index + whole + 1] << (64 - part);
        }
    }
    result
}

fn field_modulus() -> Words {
    let mut modulus = ZERO;
    modulus[FIELD_BITS / 64] |= 1 << (FIELD_BITS % 64);
    modulus[74 / 64] |= 1 << (74 % 64);
    modulus[0] |= 1;
    modulus
}

fn field_mul(a: &Words, b: &Words) -> Words {
    let modulus = field_modulus();
    let mut accumulator = ZERO;
    for index in (0..FIELD_BITS).rev() {
        accumulator = shift_left(&accumulator, 1);
        if bit(&accumulator, FIELD_BITS) {
            accumulator = xor(&accumulator, &modulus);
        }
        if bit(b, index) {
            accumulator = xor(&accumulator, a);
        }
    }
    accumulator
}

fn field_invert(a: &Words) -> Option<Words> {
    let mut u = *a;
    let mut v = field_modulus();
    let mut g1 = ONE;
    let mut g2 = ZERO;
    while u != ONE {
        let (du, dv) = (degree(&u)?, degree(&v)?);
        if du < dv {
            std::mem::swap(&mut u, &mut v);
            std::mem::swap(&mut g1, &mut g2);
        }
        let shift = degree(&u)?.abs_diff(degree(&v)?);
        u = xor(&u, &shift_left(&v, shift));
        g1 = xor(&g1, &shift_left(&g2, shift));
    }
    Some(g1)
}

fn point_add(p: &Point, q: &Point) -> Point {
    let (Some((x1, y1)), Some((x2, y2))) = (p, q) else {
        return p.or(*q);
    };
    let lambda;
    let x3;
    if x1 == x2 {
        if y1 != y2 || *x1 == ZERO {
            return None;
        }
        lambda = xor(x1, &field_mul(y1, &field_invert(x1)?));
        x3 = xor(&xor(&field_mul(&lambda, &lambda), &lambda), &ONE);
        let y3 = xor(
            &field_mul(x1, x1),
            &field_mul(&xor(&lambda, &ONE), &x3),
        );
        return Some((x3, y3));
    }
    lambda = field_mul(&xor(y1, y2), &field_invert(&xor(x1, x2))?);
    x3 = xor(
        &xor(&xor(&field_mul(&lambda, &lambda), &lambda), &xor(x1, x2)),
        &ONE,
    );
    let y3 = xor(
        &xor(&field_mul(&lambda, &xor(x1, &x3)), &x3),
        y1,
    );
    Some((x3, y3))
}

fn scalar_mul(scalar: &Words, point: &Point) -> Point {
    let top = degree(scalar)?;
    let mut result: Point = None;
    for index in (0..=top).rev() {
        result = point_add(&result, &result);
        if bit(scalar, index) {
            result = point_add(&result, point);
        }
    }
    result
}

fn compare(a: &Words, b: &Words) -> std::cmp::Ordering {
    (0..4)
        .rev()
        .map(|index| a[index].cmp(&b[index]))
        .find(|ordering| ordering.is_ne())
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn add_words(a: &Words, b: &Words) -> Words {
    let mut result = ZERO;
    let mut carry = 0u64;
    for index in 0..4 {
        let (sum, c1) = a[index].overflowing_add(b[index]);
        let (sum, c2) = sum.overflowing_add(carry);
        result[index] = sum;
        carry = u64::from(c1 | c2);
    }
    result
}

fn sub_words(a: &Words, b: &Words) -> Words {
    let mut result = ZERO;
    let mut borrow = 0u64;
    for index in 0..4 {
        let (difference, b1) = a[index].overflowing_sub(b[index]);
        let (difference, b2) = difference.overflowing_sub(borrow);
        result[index] = difference;
        borrow = u64::from(b1 | b2);
    }
    result
}

fn reduce(value: &Words, modulus: &Words) -> Words {
    let mut remainder = ZERO;
    for index in (0..256).rev() {
        remainder = shift_left(&remainder, 1);
        remainder[0] |= u64::from(bit(value, index));
        if compare(&remainder, modulus).is_ge() {
            remainder = sub_words(&remainder, modulus);
        }
    }
    remainder
}

fn mul_mod(a: &Words, b: &Words, modulus: &Words) -> Words {
    let mut result = ZERO;
    for index in (0..256).rev() {
        result = add_words(&result, &result);
        if compare(&result, modulus).is_ge() {
            result = sub_words(&result, modulus);
        }
        if bit(b, index) {
            result = add_words(&result, a);
            if compare(&result, modulus).is_ge() {
                result = sub_words(&result, modulus);
            }
        }
    }
    result
}

fn invert_mod(a: &Words, modulus: &Words) -> Words {
    let exponent = sub_words(modulus, &[2, 0, 0, 0]);
    let mut result = ONE;
    for index in (0..256).rev() {
        result = mul_mod(&result, &result, modulus);
        if bit(&exponent, index) {
            result = mul_mod(&result, a, modulus);
        }
    }
    result
}

fn is_field_element(words: &Words) -> bool {
    degree(words).is_none_or(|degree| degree < FIELD_BITS)
}

fn point_from_bytes(bytes: &[u8; PUBLIC_KEY_SIZE]) -> Option<Point> {
    let x = from_be(&bytes[..COORDINATE_SIZE])?;
    let y = from_be(&bytes[COORDINATE_SIZE..])?;
    (is_field_element(&x) && is_field_element(&y)).then_some(Some((x, y)))
}

fn on_curve(point: &Point) -> bool {
    let Some((x, y)) = point else {
        return true;
    };
    let x2 = field_mul(x, x);
    let left = xor(&field_mul(y, y), &field_mul(x, y));
    let right = xor(&xor(&field_mul(&x2, x), &x2), &curve_b());
    left == right
}

fn message_scalar(data: &[u8]) -> Option<Words> {
    let hash = from_be(&Sha256::digest(data))?;
    Some(reduce(&shift_right(&hash, 256 - 233), &order()))
}

fn add_mod(a: &Words, b: &Words, modulus: &Words) -> Words {
    let sum = add_words(a, b);
    if compare(&sum, modulus).is_ge() {
        sub_words(&sum, modulus)
    } else {
        sum
    }
}

pub fn sign(data: &[u8], private_key: &[u8], nonce: &[u8]) -> Option<[u8; SIGNATURE_SIZE]> {
    let order = order();
    let d = reduce(&from_be(private_key)?, &order);
    let k = reduce(&from_be(nonce)?, &order);
    let (x, _) = scalar_mul(&k, &Some(generator()))?;
    let r = reduce(&x, &order);
    if r == ZERO {
        return None;
    }
    let e = message_scalar(data)?;
    let s = mul_mod(
        &invert_mod(&k, &order),
        &add_mod(&e, &mul_mod(&d, &r, &order), &order),
        &order,
    );
    if s == ZERO {
        return None;
    }
    let mut signature = [0u8; SIGNATURE_SIZE];
    signature[..COORDINATE_SIZE].copy_from_slice(&to_be(&r));
    signature[COORDINATE_SIZE..].copy_from_slice(&to_be(&s));
    Some(signature)
}

pub fn public_key(private_key: &[u8]) -> Option<[u8; PUBLIC_KEY_SIZE]> {
    let order = order();
    let exponent = reduce(&from_be(private_key)?, &order);
    let (x, y) = scalar_mul(&exponent, &Some(generator()))?;
    let mut key = [0u8; PUBLIC_KEY_SIZE];
    key[..COORDINATE_SIZE].copy_from_slice(&to_be(&x));
    key[COORDINATE_SIZE..].copy_from_slice(&to_be(&y));
    Some(key)
}

pub fn verify(
    data: &[u8],
    signature: &[u8; SIGNATURE_SIZE],
    public_key: &[u8; PUBLIC_KEY_SIZE],
) -> bool {
    let Some(public) = point_from_bytes(public_key) else {
        return false;
    };
    if !on_curve(&public) {
        return false;
    }
    let order = order();
    let (Some(r), Some(s)) = (
        from_be(&signature[..COORDINATE_SIZE]),
        from_be(&signature[COORDINATE_SIZE..]),
    ) else {
        return false;
    };
    let valid = |value: &Words| *value != ZERO && compare(value, &order).is_lt();
    if !valid(&r) || !valid(&s) {
        return false;
    }
    let Some(e) = message_scalar(data) else {
        return false;
    };
    let w = invert_mod(&s, &order);
    let u1 = mul_mod(&e, &w, &order);
    let u2 = mul_mod(&r, &w, &order);
    let sum = point_add(
        &scalar_mul(&u1, &Some(generator())),
        &scalar_mul(&u2, &public),
    );
    sum.is_some_and(|(x, _)| reduce(&x, &order) == r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    struct Vector {
        raw: &'static str,
        qx: &'static str,
        qy: &'static str,
        message: &'static str,
        signature: &'static str,
    }

    const VECTORS: [Vector; 4] = [
        Vector {
            raw: "3b887ebdb67af31480c9bc39de42c6c9c1eed0bdd792cf9ec3fa01bf403557f3",
            qx: "0172244702ac31472d97c254a1f94f0fd35a3ad7819ca851412b1d1fd7e6",
            qy: "00360a5ae3c3fcb23e467bf7692ccffa8aeece1dfcc1fe3c67394eb30b9f",
            message: "message number 0",
            signature: "0020de7edcc3a42150a5c96f5eb103841fb6258e785d6c15d166ef3fdcf100f2baf028ab48096b1e2a268716519bc888a4bc343c60ea122acb94ed32",
        },
        Vector {
            raw: "3d6a1661310f61254f59f293610cc3bc1b4745d80b64c3d356619396bd36ba3d",
            qx: "01fb4e2f8ae3f1078222c5d780270eeddd67315b0e794ddd5124670c7c2a",
            qy: "011955fde6a54099768e9c364accbb2a49b5e977490bbc9af7039fc80673",
            message: "message number 1",
            signature: "002fd51a5c1366e62c676634d64667f27e871ca4d8ab94080f3429be57ab00961641bc7872102b9f462f34673fc58d9205a566344064b759ad124847",
        },
        Vector {
            raw: "2f7dbb4de82e805f4f845068a9357f3bb543ede2359abc040979a07b4922d82a",
            qx: "01651800672ffefa7f79598cecb28213be99e4fc042e3bc3fe8c3b72980d",
            qy: "0168020463b3b86ad8b9de8bc035b788b77cf48b98a0cce176f8abb0ce4b",
            message: "message number 2",
            signature: "00759deb6e47cb66ab3b5ec13e14829ff0d4115d26ecd7f359903fa14ec40087b9515eb62a3a419bc926b43377e9da1c3601f82a488e163e44fb80a9",
        },
        Vector {
            raw: "a433674e573fafb6da6ac14e10ec16e5b565ebf7908f56082898431546ebea13",
            qx: "01cf4e1e0981802c4e88ecc1656c22657ce93098f82ad66027d51b480f74",
            qy: "01c9f68a0e895518868ad05a02cce4ce42861f242910fcc1f67186905fb2",
            message: "message number 3",
            signature: "0086ec318a08cc0e0cb9a34e2ec90a540eca9cec098673f822b7f0433bc90004fd0e8b2b1c47d55b7143df8e6cee768460f8bb7653d2dcd75df51d25",
        },
    ];

    #[test]
    fn the_generator_is_on_the_curve_and_has_the_order() {
        assert!(on_curve(&Some(generator())));
        assert_eq!(scalar_mul(&order(), &Some(generator())), None);
    }

    #[test]
    fn field_inversion_gives_the_multiplicative_inverse() {
        let value = constant("0172244702ac31472d97c254a1f94f0fd35a3ad7819ca851412b1d1fd7e6");
        let inverse = field_invert(&value).unwrap();
        assert_eq!(field_mul(&value, &inverse), ONE);
    }

    #[test]
    fn public_keys_match_the_reference_implementation() {
        for vector in &VECTORS {
            let key = public_key(&hex(vector.raw)).unwrap();
            let mut expected = hex(vector.qx);
            expected.extend(hex(vector.qy));
            assert_eq!(key.to_vec(), expected);
        }
    }

    #[test]
    fn signatures_made_here_verify_with_the_matching_public_key() {
        let private_key = hex("3b887ebdb67af31480c9bc39de42c6c9c1eed0bdd792cf9ec3fa01bf403557f3");
        let nonce = hex("1234567890abcdef1234567890abcdef1234567890abcdef");
        let public = public_key(&private_key).unwrap();
        let signature = sign(b"payload", &private_key, &nonce).unwrap();
        assert!(verify(b"payload", &signature, &public));
        assert!(!verify(b"other", &signature, &public));
    }

    #[test]
    fn signatures_of_the_reference_implementation_verify() {
        for vector in &VECTORS {
            let mut key = [0u8; PUBLIC_KEY_SIZE];
            key[..COORDINATE_SIZE].copy_from_slice(&hex(vector.qx));
            key[COORDINATE_SIZE..].copy_from_slice(&hex(vector.qy));
            let signature: [u8; SIGNATURE_SIZE] = hex(vector.signature).try_into().unwrap();
            assert!(verify(vector.message.as_bytes(), &signature, &key));
            assert!(!verify(b"another message", &signature, &key));
            let mut tampered = signature;
            tampered[SIGNATURE_SIZE - 1] ^= 1;
            assert!(!verify(vector.message.as_bytes(), &tampered, &key));
        }
    }
}
