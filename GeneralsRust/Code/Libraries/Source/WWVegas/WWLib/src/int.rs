use crate::straw::Straw;
use num_bigint::{BigInt as NumBigInt, BigUint, Sign};
use num_integer::Integer;
use num_traits::{One, Signed, ToPrimitive, Zero};
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Rem, RemAssign, Sub, SubAssign};
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt {
    value: NumBigInt,
}

impl BigInt {
    pub fn new(value: NumBigInt) -> Self {
        Self { value }
    }

    pub fn zero() -> Self {
        Self {
            value: NumBigInt::zero(),
        }
    }

    pub fn one() -> Self {
        Self {
            value: NumBigInt::one(),
        }
    }

    pub fn bit_count(&self) -> u32 {
        let mag = self.value.clone().abs().to_biguint().unwrap_or_default();
        if mag.is_zero() {
            0
        } else {
            mag.bits() as u32
        }
    }

    pub fn byte_count(&self) -> usize {
        let bits = self.bit_count();
        if bits == 0 {
            1
        } else {
            ((bits + 7) / 8) as usize
        }
    }

    pub fn is_negative(&self) -> bool {
        self.value.is_negative()
    }

    pub fn abs(&self) -> Self {
        Self {
            value: self.value.abs(),
        }
    }

    pub fn exp_mod(&self, exponent: &BigInt, modulus: &BigInt) -> BigInt {
        let base = self.to_biguint();
        let exp = exponent.to_biguint();
        let modu = modulus.to_biguint();
        let result = base.modpow(&exp, &modu);
        BigInt::from_biguint(result)
    }

    pub fn inverse_mod(&self, modulus: &BigInt) -> BigInt {
        let a = self.to_biguint();
        let m = modulus.to_biguint();
        if m.is_zero() {
            return BigInt::zero();
        }
        match modinv(&a, &m) {
            Some(inv) => BigInt::from_biguint(inv),
            None => BigInt::zero(),
        }
    }

    pub fn to_biguint(&self) -> BigUint {
        self.value.clone().abs().to_biguint().unwrap_or_default()
    }

    pub fn from_biguint(value: BigUint) -> Self {
        Self {
            value: NumBigInt::from_biguint(Sign::Plus, value),
        }
    }

    pub fn from_le_bytes(bytes: &[u8]) -> Self {
        let value = BigUint::from_bytes_le(bytes);
        Self::from_biguint(value)
    }

    pub fn to_le_bytes_fixed(&self, len: usize) -> Vec<u8> {
        let mut bytes = self.to_biguint().to_bytes_le();
        if bytes.len() < len {
            bytes.resize(len, 0);
        } else if bytes.len() > len {
            bytes.truncate(len);
        }
        bytes
    }

    pub fn der_encode(&self) -> Vec<u8> {
        let mut number = self.encode_signed();
        let mut output = Vec::with_capacity(number.len() + 6);
        output.push(0x02);
        output.extend(der_length_encode(number.len() as u64));
        output.append(&mut number);
        output
    }

    pub fn der_decode(input: &[u8]) -> Option<Self> {
        if input.is_empty() || input[0] != 0x02 {
            return None;
        }
        let (len, header_len) = der_length_decode(&input[1..])?;
        let start = 1 + header_len;
        let end = start + len;
        if end > input.len() {
            return None;
        }
        let bytes = &input[start..end];
        Some(BigInt::decode_signed(bytes))
    }

    pub fn encode_signed(&self) -> Vec<u8> {
        match self.value.sign() {
            Sign::Minus => encode_twos_complement_negative(&self.value),
            _ => encode_twos_complement_positive(&self.value),
        }
    }

    pub fn decode_signed(bytes: &[u8]) -> Self {
        if bytes.is_empty() {
            return BigInt::zero();
        }
        if bytes[0] & 0x80 == 0 {
            let value = BigUint::from_bytes_be(bytes);
            BigInt::from_biguint(value)
        } else {
            let mut buf = bytes.to_vec();
            for byte in &mut buf {
                *byte = !*byte;
            }
            add_one_be(&mut buf);
            let mag = BigUint::from_bytes_be(&buf);
            BigInt {
                value: NumBigInt::from_biguint(Sign::Minus, mag),
            }
        }
    }

    pub fn randomize_bits(rng: &mut dyn Straw, total_bits: u32) -> Self {
        if total_bits == 0 {
            return BigInt::zero();
        }
        let total_bits = total_bits.min(2048);
        let nbytes = (total_bits / 8) as usize + 1;
        let mut bytes = vec![0u8; nbytes];
        fill_random_bytes(rng, &mut bytes);
        let extra_bits = (nbytes * 8) as u32 - total_bits;
        if extra_bits > 0 {
            let mask = 0xFFu8 >> extra_bits;
            let last = bytes.len() - 1;
            bytes[last] &= mask;
        }
        BigInt::from_le_bytes(&bytes)
    }

    pub fn randomize_bounded(rng: &mut dyn Straw, minval: &BigInt, maxval: &BigInt) -> Self {
        let range = maxval.clone() - minval.clone();
        let bit_count = range.bit_count();
        if bit_count == 0 {
            return minval.clone();
        }
        let mut candidate;
        loop {
            candidate = BigInt::randomize_bits(rng, bit_count);
            if candidate <= range {
                break;
            }
        }
        let mut result = candidate + minval.clone();
        result.set_bit(0);
        result
    }

    pub fn set_bit(&mut self, index: u32) {
        if index == 0 {
            if self.value.is_even() {
                self.value += NumBigInt::one();
            }
            return;
        }
        let mut mag = self.to_biguint();
        mag.set_bit(index as u64, true);
        self.value = NumBigInt::from_biguint(Sign::Plus, mag);
    }

    pub fn mod_u16(&self, divisor: u16) -> u16 {
        if divisor == 0 {
            return 0;
        }
        let m = BigUint::from(divisor as u32);
        let rem = self.to_biguint() % m;
        rem.to_u16().unwrap_or(0)
    }

    pub fn is_prime(&self) -> bool {
        if self.value.is_even() {
            return false;
        }
        if is_small_prime(self) {
            return true;
        }
        if !small_divisors_test(self) {
            return false;
        }
        fermat_test(self, 2)
    }
}

impl Default for BigInt {
    fn default() -> Self {
        BigInt::zero()
    }
}

impl From<u64> for BigInt {
    fn from(value: u64) -> Self {
        BigInt {
            value: NumBigInt::from(value),
        }
    }
}

impl From<i64> for BigInt {
    fn from(value: i64) -> Self {
        BigInt {
            value: NumBigInt::from(value),
        }
    }
}

impl Add for BigInt {
    type Output = BigInt;
    fn add(self, rhs: BigInt) -> Self::Output {
        BigInt {
            value: self.value + rhs.value,
        }
    }
}

impl AddAssign for BigInt {
    fn add_assign(&mut self, rhs: BigInt) {
        self.value += rhs.value;
    }
}

impl Sub for BigInt {
    type Output = BigInt;
    fn sub(self, rhs: BigInt) -> Self::Output {
        BigInt {
            value: self.value - rhs.value,
        }
    }
}

impl SubAssign for BigInt {
    fn sub_assign(&mut self, rhs: BigInt) {
        self.value -= rhs.value;
    }
}

impl Mul for BigInt {
    type Output = BigInt;
    fn mul(self, rhs: BigInt) -> Self::Output {
        BigInt {
            value: self.value * rhs.value,
        }
    }
}

impl MulAssign for BigInt {
    fn mul_assign(&mut self, rhs: BigInt) {
        self.value *= rhs.value;
    }
}

impl Div for BigInt {
    type Output = BigInt;
    fn div(self, rhs: BigInt) -> Self::Output {
        BigInt {
            value: self.value / rhs.value,
        }
    }
}

impl DivAssign for BigInt {
    fn div_assign(&mut self, rhs: BigInt) {
        self.value /= rhs.value;
    }
}

impl Rem for BigInt {
    type Output = BigInt;
    fn rem(self, rhs: BigInt) -> Self::Output {
        BigInt {
            value: self.value % rhs.value,
        }
    }
}

impl RemAssign for BigInt {
    fn rem_assign(&mut self, rhs: BigInt) {
        self.value %= rhs.value;
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

fn encode_twos_complement_positive(value: &NumBigInt) -> Vec<u8> {
    let mag = value.clone().abs().to_biguint().unwrap_or_default();
    let mut bytes = mag.to_bytes_be();
    if bytes.is_empty() {
        bytes.push(0);
    }
    if bytes[0] & 0x80 != 0 {
        bytes.insert(0, 0x00);
    }
    bytes
}

fn encode_twos_complement_negative(value: &NumBigInt) -> Vec<u8> {
    let mag = value.clone().abs().to_biguint().unwrap_or_default();
    let mut bytes = mag.to_bytes_be();
    if bytes.is_empty() {
        bytes.push(0);
    }
    for byte in &mut bytes {
        *byte = !*byte;
    }
    add_one_be(&mut bytes);
    if bytes[0] & 0x80 == 0 {
        bytes.insert(0, 0xFF);
    }
    bytes
}

fn add_one_be(bytes: &mut [u8]) {
    let mut carry = 1u8;
    for byte in bytes.iter_mut().rev() {
        let (res, overflow) = byte.overflowing_add(carry);
        *byte = res;
        if !overflow {
            carry = 0;
            break;
        }
    }
}

fn der_length_encode(length: u64) -> Vec<u8> {
    if length <= i8::MAX as u64 {
        vec![length as u8]
    } else {
        let mut bytes = Vec::new();
        let mut value = length;
        while value > 0 {
            bytes.push((value & 0xFF) as u8);
            value >>= 8;
        }
        bytes.reverse();
        let mut output = Vec::with_capacity(bytes.len() + 1);
        output.push((bytes.len() as u8) | 0x80);
        output.extend(bytes);
        output
    }
}

fn der_length_decode(input: &[u8]) -> Option<(usize, usize)> {
    if input.is_empty() {
        return None;
    }
    if input[0] & 0x80 == 0 {
        return Some((input[0] as usize, 1));
    }
    let len_bytes = (input[0] & 0x7F) as usize;
    if len_bytes == 0 || len_bytes > 4 || input.len() < 1 + len_bytes {
        return None;
    }
    let mut length = 0usize;
    for i in 0..len_bytes {
        length = (length << 8) | (input[1 + i] as usize);
    }
    Some((length, 1 + len_bytes))
}

fn fill_random_bytes(rng: &mut dyn Straw, buffer: &mut [u8]) {
    let mut offset = 0usize;
    while offset < buffer.len() {
        let got = rng.get(&mut buffer[offset..]);
        if got <= 0 {
            break;
        }
        offset += got as usize;
    }
}

fn prime_table() -> &'static Vec<u16> {
    static PRIMES: OnceLock<Vec<u16>> = OnceLock::new();
    PRIMES.get_or_init(|| {
        let limit = 32719usize;
        let mut sieve = vec![true; limit + 1];
        sieve[0] = false;
        if limit >= 1 {
            sieve[1] = false;
        }
        let mut p = 2usize;
        while p * p <= limit {
            if sieve[p] {
                let mut multiple = p * p;
                while multiple <= limit {
                    sieve[multiple] = false;
                    multiple += p;
                }
            }
            p += 1;
        }
        let mut primes = Vec::new();
        for (i, is_prime) in sieve.into_iter().enumerate() {
            if is_prime {
                primes.push(i as u16);
            }
        }
        primes
    })
}

fn is_small_prime(candidate: &BigInt) -> bool {
    if candidate.value.is_negative() {
        return false;
    }
    let primes = prime_table();
    let max = *primes.last().unwrap_or(&0) as u64;
    let value = candidate.to_biguint();
    if value.bits() > 16 {
        return false;
    }
    let val = value.to_u64().unwrap_or(u64::MAX);
    if val > max {
        return false;
    }
    primes.binary_search(&(val as u16)).is_ok()
}

fn small_divisors_test(candidate: &BigInt) -> bool {
    let primes = prime_table();
    for &prime in primes {
        if candidate.mod_u16(prime) == 0 {
            return false;
        }
    }
    true
}

fn fermat_test(candidate: &BigInt, rounds: usize) -> bool {
    let primes = prime_table();
    if candidate.value.is_negative() {
        return false;
    }
    let cand = candidate.to_biguint();
    if cand.is_zero() {
        return false;
    }
    let cand_minus_one = &cand - BigUint::one();
    for i in 0..rounds.min(primes.len()) {
        let base = BigUint::from(primes[i] as u32);
        let res = base.modpow(&cand_minus_one, &cand);
        if res != BigUint::one() {
            return false;
        }
    }
    true
}

fn modinv(a: &BigUint, modulus: &BigUint) -> Option<BigUint> {
    let mut t = NumBigInt::zero();
    let mut new_t = NumBigInt::one();
    let mut r = NumBigInt::from_biguint(Sign::Plus, modulus.clone());
    let mut new_r = NumBigInt::from_biguint(Sign::Plus, a.clone());

    while !new_r.is_zero() {
        let quotient = &r / &new_r;
        let temp_t = t.clone() - &quotient * &new_t;
        t = new_t;
        new_t = temp_t;
        let temp_r = r.clone() - &quotient * &new_r;
        r = new_r;
        new_r = temp_r;
    }

    if r != NumBigInt::one() {
        return None;
    }

    if t.is_negative() {
        t += NumBigInt::from_biguint(Sign::Plus, modulus.clone());
    }
    t.to_biguint()
}

pub struct RemainderTable {
    table: Vec<u16>,
    has_zero_entry: bool,
}

impl RemainderTable {
    pub fn new(value: &BigInt) -> Self {
        let primes = prime_table();
        let mut table = Vec::with_capacity(primes.len());
        let mut has_zero = false;
        for &prime in primes {
            let rem = value.mod_u16(prime);
            if rem == 0 {
                has_zero = true;
            }
            table.push(rem);
        }
        Self {
            table,
            has_zero_entry: has_zero,
        }
    }

    pub fn has_zero(&self) -> bool {
        self.has_zero_entry
    }

    pub fn increment(&mut self, increment: u16) {
        let primes = prime_table();
        self.has_zero_entry = false;
        for (i, prime) in primes.iter().enumerate() {
            let mut rem = self.table[i] as u32 + increment as u32;
            rem %= *prime as u32;
            let rem = rem as u16;
            if rem == 0 {
                self.has_zero_entry = true;
            }
            self.table[i] = rem;
        }
    }
}

pub fn generate_prime(rng: &mut dyn Straw, pbits: u32) -> BigInt {
    let min_q = BigInt::from(1u64) << (pbits - 2);
    let max_q = (BigInt::from(1u64) << (pbits - 1)) - BigInt::from(1u64);

    loop {
        let mut q = BigInt::randomize_bounded(rng, &min_q, &max_q);
        let mut p = q.clone() * BigInt::from(2u64) + BigInt::from(1u64);

        let mut rt_q = RemainderTable::new(&q);
        let mut rt_p = RemainderTable::new(&p);

        while rt_q.has_zero() || rt_p.has_zero() || !q.is_prime() || !p.is_prime() {
            q += BigInt::from(2u64);
            p += BigInt::from(4u64);
            if q > max_q {
                break;
            }
            rt_q.increment(2);
            rt_p.increment(4);
        }

        if q <= max_q {
            return p;
        }
    }
}

impl std::ops::Shl<u32> for BigInt {
    type Output = BigInt;
    fn shl(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value << rhs,
        }
    }
}

impl std::ops::Shr<u32> for BigInt {
    type Output = BigInt;
    fn shr(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value >> rhs,
        }
    }
}

impl std::ops::ShlAssign<u32> for BigInt {
    fn shl_assign(&mut self, rhs: u32) {
        self.value <<= rhs;
    }
}

impl std::ops::ShrAssign<u32> for BigInt {
    fn shr_assign(&mut self, rhs: u32) {
        self.value >>= rhs;
    }
}

impl std::ops::BitOrAssign<u32> for BigInt {
    fn bitor_assign(&mut self, rhs: u32) {
        let mut mag = self.to_biguint();
        mag |= BigUint::from(rhs);
        self.value = NumBigInt::from_biguint(Sign::Plus, mag);
    }
}

impl std::ops::BitOr<u32> for BigInt {
    type Output = BigInt;
    fn bitor(self, rhs: u32) -> Self::Output {
        let mut mag = self.to_biguint();
        mag |= BigUint::from(rhs);
        BigInt::from_biguint(mag)
    }
}

impl std::ops::BitAnd<u32> for BigInt {
    type Output = BigInt;
    fn bitand(self, rhs: u32) -> Self::Output {
        let mag = self.to_biguint() & BigUint::from(rhs);
        BigInt::from_biguint(mag)
    }
}

impl std::ops::Add<u32> for BigInt {
    type Output = BigInt;
    fn add(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value + NumBigInt::from(rhs),
        }
    }
}

impl std::ops::Sub<u32> for BigInt {
    type Output = BigInt;
    fn sub(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value - NumBigInt::from(rhs),
        }
    }
}

impl std::ops::Mul<u32> for BigInt {
    type Output = BigInt;
    fn mul(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value * NumBigInt::from(rhs),
        }
    }
}

impl std::ops::AddAssign<u32> for BigInt {
    fn add_assign(&mut self, rhs: u32) {
        self.value += NumBigInt::from(rhs);
    }
}

impl std::ops::SubAssign<u32> for BigInt {
    fn sub_assign(&mut self, rhs: u32) {
        self.value -= NumBigInt::from(rhs);
    }
}

impl std::ops::MulAssign<u32> for BigInt {
    fn mul_assign(&mut self, rhs: u32) {
        self.value *= NumBigInt::from(rhs);
    }
}

impl std::ops::Div<u32> for BigInt {
    type Output = BigInt;
    fn div(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value / NumBigInt::from(rhs),
        }
    }
}

impl std::ops::Rem<u32> for BigInt {
    type Output = BigInt;
    fn rem(self, rhs: u32) -> Self::Output {
        BigInt {
            value: self.value % NumBigInt::from(rhs),
        }
    }
}

impl std::ops::DivAssign<u32> for BigInt {
    fn div_assign(&mut self, rhs: u32) {
        self.value /= NumBigInt::from(rhs);
    }
}

impl std::ops::RemAssign<u32> for BigInt {
    fn rem_assign(&mut self, rhs: u32) {
        self.value %= NumBigInt::from(rhs);
    }
}

impl std::fmt::Display for BigInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}
