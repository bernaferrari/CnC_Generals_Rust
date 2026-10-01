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

        fn get_contained_objects(&self) -> std::borrow::Cow<'_, [ObjectID]> {
            std::borrow::Cow::Borrowed(&self.ids)
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
    fn contain_trait_add_to_contain_increases_contained_count() {
        let passenger = Object::new_test(42, 100.0);
        let mut contain: Box<dyn ContainModuleInterface> = Box::new(TestContain {
            ids: Vec::new(),
            max: 4,
        });

        assert!(contain.is_valid_container_for(&passenger, true));
        assert_eq!(contain.get_contained_count(), 0);
        assert!(contain.get_contained_objects().is_empty());

        contain.add_to_contain(&passenger).expect("add passenger");

        assert_eq!(contain.get_contained_count(), 1);
        assert_eq!(contain.get_contained_objects().to_vec(), vec![42]);
        assert!(contain.is_valid_container_for(&passenger, true));
    }

    #[test]
    fn contain_trait_full_container_rejects_add() {
        let passenger = Object::new_test(7, 100.0);
        let mut contain: Box<dyn ContainModuleInterface> = Box::new(TestContain {
            ids: vec![1, 2, 3, 4],
            max: 4,
        });

        assert!(!contain.is_valid_container_for(&passenger, true));
        assert!(
            contain.add_to_contain(&passenger).is_err(),
            "add_to_contain beyond capacity must fail, not pretend success"
        );
        assert_eq!(contain.get_contained_count(), 4);
    }
}
