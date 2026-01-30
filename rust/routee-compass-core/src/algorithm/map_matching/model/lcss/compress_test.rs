#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_no_consecutive() {
        let points = vec![1, 3, 5, 7];
        let compressed = compress(points.clone());
        assert_eq!(compressed, points);
    }

    #[test]
    fn test_compress_all_consecutive() {
        let points = vec![1, 2, 3, 4, 5];
        let compressed = compress(points);
        assert_eq!(compressed, vec![3]);
    }

    #[test]
    fn test_compress_mixed() {
        let points = vec![1, 2, 3, 6, 7, 8, 10];
        let compressed = compress(points);
        assert_eq!(compressed, vec![2, 7, 10]);
    }

    #[test]
    fn test_compress_empty() {
        let points: Vec<usize> = vec![];
        let compressed = compress(points);
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_compress_single() {
        let points = vec![5];
        let compressed = compress(points);
        assert_eq!(compressed, vec![5]);
    }

    #[test]
    fn test_compress_duplicates() {
        // Although input should be sorted/unique typically, compress handles sorted.
        // If we have [2, 2, 3, 3].
        // i=0, v=2. key=0-2=-2.
        // i=1, v=2. key=1-2=-1. -> Different group.
        // So [2, 2] are not grouped.
        // The Python logic groups by (index - value).
        // So strictly strictly it groups consecutive *integers*.
        // If we have [1, 2, 4, 5].
        // 1, 2 -> group 1. Mid 2? (len 2, mid index 1 -> value 2).
        // 4, 5 -> group 2. Mid 5? (len 2, mid index 1 -> value 5).

        let points = vec![1, 2, 4, 5];
        let compressed = compress(points);
        // 1,2 -> 2. 4,5 -> 5.
        assert_eq!(compressed, vec![2, 5]);

        // [1, 2, 3] -> 2.
    }
}
