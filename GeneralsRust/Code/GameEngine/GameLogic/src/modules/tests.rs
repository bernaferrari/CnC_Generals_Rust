// Module interface unit tests
//
// Split from `modules.rs` for module-size parity.
// Observable behavior is unchanged.

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestContain {
        ids: Vec<ObjectID>,
        max: usize,
    }

    impl ContainModuleInterface for TestContain {
        fn can_contain(&self, _object_id: ObjectID) -> bool {
            self.ids.len() < self.max
        }

        fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
            if !self.can_contain(object_id) {
                return Err("container full".into());
            }
            self.ids.push(object_id);
            Ok(())
        }

        fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
            self.ids.retain(|id| *id != object_id);
            Ok(())
        }

        fn get_contained_objects(&self) -> &[ObjectID] {
            &self.ids
        }

        fn get_contained_count(&self) -> usize {
            self.ids.len()
        }

        fn get_max_capacity(&self) -> usize {
            self.max
        }

        fn is_valid_container_for(&self, _obj: &Object, check_capacity: bool) -> bool {
            if check_capacity {
                self.ids.len() < self.max
            } else {
                true
            }
        }
    }

    #[test]
    fn ext_add_to_contain_increases_contained_count() {
        let passenger = Object::new_test(42, 100.0);
        let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(TestContain {
            ids: Vec::new(),
            max: 4,
        }));

        assert!(contain.is_valid_container_for(&passenger, true));
        assert_eq!(contain.get_contained_count(), 0);
        assert!(contain.get_contained_objects().is_empty());

        contain.add_to_contain(&passenger);

        assert_eq!(contain.get_contained_count(), 1);
        assert_eq!(contain.get_contained_objects(), vec![42]);
        assert!(contain.is_valid_container_for(&passenger, true));
    }

    #[test]
    fn ext_poison_is_fail_closed() {
        let passenger = Object::new_test(7, 100.0);
        let contain: Arc<Mutex<dyn ContainModuleInterface>> = Arc::new(Mutex::new(TestContain {
            ids: Vec::new(),
            max: 4,
        }));
        let poisoned = Arc::clone(&contain);
        let join = std::thread::spawn(move || {
            let _guard = poisoned.lock().expect("lock for poison");
            panic!("intentional contain mutex poison");
        })
        .join();
        assert!(join.is_err(), "worker must poison the mutex");

        assert!(
            !contain.is_valid_container_for(&passenger, true),
            "poisoned is_valid_container_for must fail-closed"
        );
        assert_eq!(contain.get_contained_count(), 0);
        assert!(contain.get_contained_objects().is_empty());
        contain.add_to_contain(&passenger);
        assert_eq!(
            contain.get_contained_count(),
            0,
            "poisoned add_to_contain must not pretend success"
        );
    }

    fn contain_ext_impl_source() -> &'static str {
        const SRC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/modules.rs"));
        const MARKER: &str =
            "impl ContainModuleInterfaceExt for Arc<Mutex<dyn ContainModuleInterface>> {";
        let start = SRC
            .find(MARKER)
            .expect("ContainModuleInterfaceExt impl missing");
        let after = &SRC[start + MARKER.len()..];
        let end = after.find("\nimpl ").unwrap_or(after.len());
        &SRC[start..start + MARKER.len() + end]
    }

    fn method_body<'a>(block: &'a str, signature: &str) -> &'a str {
        let start = block
            .find(signature)
            .unwrap_or_else(|| panic!("missing method {signature}"));
        let rest = &block[start..];
        let brace = rest.find('{').unwrap_or_else(|| panic!("{signature} body"));
        let bytes = rest.as_bytes();
        let mut depth = 0usize;
        for (idx, byte) in bytes.iter().enumerate().skip(brace) {
            match *byte {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &rest[brace..=idx];
                    }
                }
                _ => {}
            }
        }
        panic!("unclosed body for {signature}");
    }

    #[test]
    fn ext_critical_methods_use_blocking_lock_not_try_lock() {
        let block = contain_ext_impl_source();
        for signature in [
            "fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool",
            "fn add_to_contain(&self, obj: &Object)",
            "fn get_contained_objects(&self) -> Vec<ObjectID>",
            "fn get_contained_count(&self) -> usize",
        ] {
            let body = method_body(block, signature);
            assert!(
                body.contains(".lock()"),
                "{signature} must use blocking lock(), body={body}"
            );
            assert!(
                !body.contains(".try_lock()"),
                "{signature} must not use try_lock(), body={body}"
            );
        }
    }
}
