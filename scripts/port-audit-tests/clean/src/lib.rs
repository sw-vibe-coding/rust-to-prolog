pub fn ok() -> i32 {
    42
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_long_test_body_is_allowed() {
        let mut x = 0;
        // deliberately > 50 lines to prove #[test] is exempt.
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1; x += 1;
        assert!(x > 0);
    }
}
