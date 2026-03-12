// Auto-generated C++ compatibility shim for tree
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
pub struct TreeNode<T> {
    pub value: T,
    pub children: Vec<Rc<RefCell<TreeNode<T>>>>,
}

impl<T> TreeNode<T> {
    pub fn new(value: T) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            value,
            children: Vec::new(),
        }))
    }

    pub fn add_child(parent: &Rc<RefCell<Self>>, child: Rc<RefCell<Self>>) {
        parent.borrow_mut().children.push(child);
    }
}
