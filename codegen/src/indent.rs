pub(super) trait IndentNode: Sized {
    type Output;

    fn level(&self) -> usize;
    fn into_output(self) -> Self::Output;
    fn try_adopt(&mut self, children: Vec<Self::Output>) -> Result<(), String>;

    fn try_finish_siblings(siblings: Vec<Self::Output>) -> Result<Vec<Self::Output>, String> {
        Ok(siblings)
    }
}

pub(super) struct Indent;

impl Indent {
    pub(super) fn try_build<N: IndentNode>(nodes: Vec<N>) -> Result<Vec<N::Output>, String> {
        let mut stack: Vec<N> = Vec::new();
        let mut roots: Vec<N::Output> = Vec::new();
        for node in nodes {
            while stack.last().is_some_and(|top| node.level() < top.level()) {
                Self::try_collapse(&mut stack, &mut roots)?;
            }
            stack.push(node);
        }
        while !stack.is_empty() {
            Self::try_collapse(&mut stack, &mut roots)?;
        }
        Ok(roots)
    }

    fn try_collapse<N: IndentNode>(
        stack: &mut Vec<N>,
        roots: &mut Vec<N::Output>,
    ) -> Result<(), String> {
        let leaf_level = stack.last().unwrap().level();
        let mut siblings = Vec::new();
        while stack.last().is_some_and(|node| node.level() == leaf_level) {
            siblings.push(stack.pop().unwrap().into_output());
        }
        siblings.reverse();
        let siblings = N::try_finish_siblings(siblings)?;
        match stack.last_mut() {
            Some(parent) => parent.try_adopt(siblings)?,
            None => roots.extend(siblings),
        }
        Ok(())
    }
}
