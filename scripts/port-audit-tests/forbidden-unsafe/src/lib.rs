pub fn bad() {
    unsafe {
        let _p: *const u8 = std::ptr::null();
    }
}
