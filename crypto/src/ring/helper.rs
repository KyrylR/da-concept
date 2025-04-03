/// Computes the greatest common divisor of two numbers.
pub fn gcd(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let temp = b;
        b = a % b;
        a = temp;
    }
    a
}

/// Finds (g, x, y) such that ax + by = g = gcd(a, b).
pub fn extended_gcd(a: i64, b: i64) -> (i64, i64, i64) {
    if a == 0 {
        if b.is_negative() {
            return (-b, 0, -1);
        }

        return (b, 0, 1);
    }

    let (g, x1, y1) = extended_gcd(b % a, a);
    let x = y1 - (b / a) * x1;
    let y = x1;
    (g, x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_gcd() {
        assert_eq!(gcd(1, 6), 1);
        assert_eq!(gcd(5, 6), 1);
        assert_eq!(gcd(2, 6), 2);
        assert_eq!(gcd(3, 6), 3);
        assert_eq!(gcd(4, 6), 2);
        assert_eq!(gcd(6, 6), 6);
        assert_eq!(gcd(7, 6), 1);
        assert_eq!(gcd(10, 0), 10);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(0, 0), 0);
        assert_eq!(gcd(54, 24), 6);
    }

    #[test]
    fn test_equivalence_with_extended_gcd() {
        let (g, _, _) = extended_gcd(12, 8);
        assert_eq!(g, { gcd(12, 8) });
    }

    #[test]
    fn test_extended_gcd_basic() {
        let (g, x, y) = extended_gcd(12, 8);
        assert_eq!(g, 4);
        assert_eq!(12 * x + 8 * y, g);

        let (g, x, y) = extended_gcd(17, 13);
        assert_eq!(g, 1);
        assert_eq!(17 * x + 13 * y, g);
    }

    #[test]
    fn test_extended_gcd_zero() {
        let (g, x, y) = extended_gcd(0, 15);
        assert_eq!(g, 15);
        assert_eq!(x, 0);
        assert_eq!(y, 1);
        assert_eq!(15 * y, g);

        let (g, x, _y) = extended_gcd(15, 0);
        assert_eq!(g, 15);
        assert_eq!(15 * x, g);
    }

    #[test]
    fn test_extended_gcd_negative() {
        let (g, x, y) = extended_gcd(-15, 10);
        assert_eq!(g, 5);
        assert_eq!(-15 * x + 10 * y, g);

        let (g, x, y) = extended_gcd(-12, -9);
        assert_eq!(g, 3);
        assert_eq!(-12 * x + (-9) * y, g);
    }

    #[test]
    fn test_extended_gcd_large() {
        let (g, x, y) = extended_gcd(240, 46);
        assert_eq!(g, 2);
        assert_eq!(240 * x + 46 * y, g);

        let (g, x, y) = extended_gcd(1001, 103);
        assert_eq!(g, 1);
        assert_eq!(1001 * x + 103 * y, g);
    }
}
