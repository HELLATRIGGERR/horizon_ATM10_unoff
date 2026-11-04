//! The part of `java.math.BigInteger` MEGA's bulk cell uses, for the
//! non-negative counts it keeps: a unit count and the factor of its stored
//! item (`MEGAComponents.BULK_CELL_UNIT_COUNT`, `BULK_CELL_UNIT_FACTOR`, saved
//! as decimal strings by `BIG_INTEGER_CODEC`).
use std::cmp::Ordering;
use std::fmt;

/// A non-negative integer of any size: its base 2^32 digits, least
/// significant first, with no zero at the top.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Big {
    limbs: Vec<u32>,
}

impl Big {
    pub fn zero() -> Self {
        Self::default()
    }
    pub fn one() -> Self {
        Self::from(1u64)
    }
    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }
    fn trim(mut self) -> Self {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        self
    }
    /// `new BigInteger(str)`: an optional sign, then decimal digits. A
    /// negative value, which no bulk cell writes, reads as zero.
    pub fn parse(text: &str) -> Option<Self> {
        let (negative, digits) = match text.as_bytes().first() {
            Some(b'-') => (true, &text[1..]),
            Some(b'+') => (false, &text[1..]),
            _ => (false, text),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut value = Self::zero();
        for chunk in digits.as_bytes().chunks(9) {
            let part: u32 = std::str::from_utf8(chunk).ok()?.parse().ok()?;
            value = value.mul_small(10u32.pow(chunk.len() as u32)).add(&Self::from(u64::from(part)));
        }
        Some(if negative { Self::zero() } else { value })
    }
    fn mul_small(&self, factor: u32) -> Self {
        let mut carry = 0u64;
        let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
        for &limb in &self.limbs {
            let product = u64::from(limb) * u64::from(factor) + carry;
            limbs.push(product as u32);
            carry = product >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        Self { limbs }.trim()
    }
    /// `add`.
    pub fn add(&self, other: &Self) -> Self {
        let mut limbs = Vec::with_capacity(self.limbs.len().max(other.limbs.len()) + 1);
        let mut carry = 0u64;
        for i in 0..self.limbs.len().max(other.limbs.len()) {
            let sum = u64::from(self.limbs.get(i).copied().unwrap_or(0)) + u64::from(other.limbs.get(i).copied().unwrap_or(0)) + carry;
            limbs.push(sum as u32);
            carry = sum >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        Self { limbs }.trim()
    }
    /// `subtract(other).max(ZERO)`: zero when `other` is the larger.
    pub fn saturating_sub(&self, other: &Self) -> Self {
        if *self <= *other {
            return Self::zero();
        }
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow = 0i64;
        for i in 0..self.limbs.len() {
            let mut difference = i64::from(self.limbs[i]) - i64::from(other.limbs.get(i).copied().unwrap_or(0)) - borrow;
            borrow = 0;
            if difference < 0 {
                difference += 1 << 32;
                borrow = 1;
            }
            limbs.push(difference as u32);
        }
        Self { limbs }.trim()
    }
    /// `multiply`.
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &b) in other.limbs.iter().enumerate() {
                let current = u64::from(limbs[i + j]) + u64::from(a) * u64::from(b) + carry;
                limbs[i + j] = current as u32;
                carry = current >> 32;
            }
            let mut k = i + other.limbs.len();
            while carry > 0 {
                let current = u64::from(limbs[k]) + carry;
                limbs[k] = current as u32;
                carry = current >> 32;
                k += 1;
            }
        }
        Self { limbs }.trim()
    }
    fn bits(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(top) => (self.limbs.len() - 1) * 32 + (32 - top.leading_zeros() as usize),
        }
    }
    fn bit(&self, i: usize) -> bool {
        self.limbs.get(i / 32).is_some_and(|limb| limb >> (i % 32) & 1 == 1)
    }
    /// `divideAndRemainder`, truncating as Java does (both are non-negative
    /// here). Dividing by zero panics, as Java throws.
    pub fn div_rem(&self, divisor: &Self) -> (Self, Self) {
        assert!(!divisor.is_zero(), "BigInteger divide by zero");
        if self < divisor {
            return (Self::zero(), self.clone());
        }
        if divisor.limbs.len() == 1 {
            let d = u64::from(divisor.limbs[0]);
            let mut quotient = vec![0u32; self.limbs.len()];
            let mut remainder = 0u64;
            for i in (0..self.limbs.len()).rev() {
                let current = (remainder << 32) | u64::from(self.limbs[i]);
                quotient[i] = (current / d) as u32;
                remainder = current % d;
            }
            return (Self { limbs: quotient }.trim(), Self::from(remainder));
        }
        let mut quotient = vec![0u32; self.limbs.len()];
        let mut remainder = Self::zero();
        for i in (0..self.bits()).rev() {
            remainder = remainder.mul_small(2);
            if self.bit(i) {
                remainder = remainder.add(&Self::one());
            }
            if remainder >= *divisor {
                remainder = remainder.saturating_sub(divisor);
                quotient[i / 32] |= 1 << (i % 32);
            }
        }
        (Self { limbs: quotient }.trim(), remainder)
    }
    /// `divide`.
    pub fn div(&self, divisor: &Self) -> Self {
        self.div_rem(divisor).0
    }
    /// `remainder`.
    pub fn rem(&self, divisor: &Self) -> Self {
        self.div_rem(divisor).1
    }
    /// `CompressionChain.clamp(value, limit)`: `value.min(limit).longValue()`.
    pub fn clamp_to(&self, limit: u64) -> u64 {
        match self.limbs.len() {
            0 => 0,
            1 => u64::from(self.limbs[0]).min(limit),
            2 => (u64::from(self.limbs[1]) << 32 | u64::from(self.limbs[0])).min(limit),
            _ => limit,
        }
    }
}

impl From<u64> for Big {
    fn from(value: u64) -> Self {
        Self { limbs: vec![value as u32, (value >> 32) as u32] }.trim()
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        self.limbs.len().cmp(&other.limbs.len()).then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}
impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// `BigInteger.toString()`: decimal digits.
impl fmt::Display for Big {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }
        let billion = Self::from(1_000_000_000u64);
        let mut parts = Vec::new();
        let mut rest = self.clone();
        while !rest.is_zero() {
            let (quotient, remainder) = rest.div_rem(&billion);
            parts.push(remainder.clamp_to(u64::MAX));
            rest = quotient;
        }
        write!(f, "{}", parts.pop().unwrap_or(0))?;
        for part in parts.iter().rev() {
            write!(f, "{part:09}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Big;

    fn big(text: &str) -> Big {
        Big::parse(text).unwrap()
    }

    /// The arithmetic against `u128` where it fits, and decimal text past it
    /// (values worked out by hand from 2^64 and 9^30).
    #[test]
    fn arithmetic_matches_u128_and_decimal_text() {
        let samples: [u128; 8] = [0, 1, 9, 81, 4_294_967_296, 18_446_744_073_709_551_615, 18_446_744_073_709_551_616, 170_141_183_460_469_231_731_687_303_715_884_105_727];
        for &a in &samples {
            assert_eq!(big(&a.to_string()).to_string(), a.to_string());
            for &b in &samples {
                let (x, y) = (big(&a.to_string()), big(&b.to_string()));
                assert_eq!(x.add(&y).to_string(), (a + b).to_string());
                assert_eq!(x.saturating_sub(&y).to_string(), a.saturating_sub(b).to_string());
                assert_eq!(x.cmp(&y), a.cmp(&b));
                if let Some(product) = a.checked_mul(b) {
                    assert_eq!(x.mul(&y).to_string(), product.to_string());
                }
                if b > 0 {
                    assert_eq!(x.div(&y).to_string(), (a / b).to_string(), "{a} / {b}");
                    assert_eq!(x.rem(&y).to_string(), (a % b).to_string(), "{a} % {b}");
                }
            }
        }
        // 9^30 = 42391158275216203514294433201, and 2^64 × 2^64 = 2^128.
        let nine = Big::from(9);
        let power = (0..30).fold(Big::one(), |acc, _| acc.mul(&nine));
        assert_eq!(power.to_string(), "42391158275216203514294433201");
        assert_eq!(power.div(&nine.mul(&nine)).to_string(), "523347633027360537213511521");
        let two64 = big("18446744073709551616");
        assert_eq!(two64.mul(&two64).to_string(), "340282366920938463463374607431768211456");
        assert_eq!(two64.mul(&two64).div(&two64), two64);
        assert_eq!(power.clamp_to(i64::MAX as u64), i64::MAX as u64);
        assert_eq!(Big::from(77).clamp_to(1 << 42), 77);
    }

    /// `new BigInteger(str)`: a sign, then digits; nothing else.
    #[test]
    fn parses_java_decimal_text() {
        assert_eq!(big("+42"), Big::from(42));
        assert_eq!(big("000123"), Big::from(123));
        assert_eq!(big("-5"), Big::zero(), "a negative count reads as zero");
        for bad in ["", "-", "1.5", "12a", " 1", "0x10"] {
            assert!(Big::parse(bad).is_none(), "{bad:?}");
        }
    }
}
