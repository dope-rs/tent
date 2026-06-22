pub(super) trait IndentNode: Sized {
    type Output;

    fn level(&self) -> usize;
    fn into_output(self) -> Self::Output;
    fn adopt(&mut self, children: Vec<Self::Output>);

    fn finish_siblings(siblings: Vec<Self::Output>) -> Vec<Self::Output> {
        siblings
    }
}

pub(super) struct Indent;

impl Indent {
    pub(super) fn build<N: IndentNode>(nodes: Vec<N>) -> Vec<N::Output> {
        let mut stack: Vec<N> = Vec::new();
        let mut roots: Vec<N::Output> = Vec::new();
        for node in nodes {
            while stack.last().is_some_and(|top| node.level() < top.level()) {
                Self::collapse(&mut stack, &mut roots);
            }
            stack.push(node);
        }
        while !stack.is_empty() {
            Self::collapse(&mut stack, &mut roots);
        }
        roots
    }

    fn collapse<N: IndentNode>(stack: &mut Vec<N>, roots: &mut Vec<N::Output>) {
        let leaf_level = stack.last().unwrap().level();
        let mut siblings = Vec::new();
        while stack.last().is_some_and(|node| node.level() == leaf_level) {
            siblings.push(stack.pop().unwrap().into_output());
        }
        siblings.reverse();
        let siblings = N::finish_siblings(siblings);
        match stack.last_mut() {
            Some(parent) => parent.adopt(siblings),
            None => roots.extend(siblings),
        }
    }
}
