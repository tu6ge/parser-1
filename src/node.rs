use std::any::Any;

pub trait Node: Any {
    fn children(&mut self) -> Vec<&mut dyn Node> {
        vec![]
    }
}

impl<N: Node> Node for Vec<N> {
    fn children(&mut self) -> Vec<&mut dyn Node> {
        self.iter_mut().map(|a| a as &mut dyn Node).collect()
    }
}
