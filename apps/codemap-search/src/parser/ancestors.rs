//! Ancestor walks over syntax trees.

use tree_sitter::Node;

/// The ancestors of `node`, nearest first: the nodes repeated [`Node::parent`] calls return.
/// Each `Node::parent` call descends from the root again, so a walk of `depth` steps costs
/// `depth` descents; these all come from the one descent that finds the first of them.
pub(crate) fn ancestors(node: Node<'_>) -> Ancestors<'_> {
    Ancestors { node, path: None }
}

pub(crate) struct Ancestors<'tree> {
    node: Node<'tree>,
    /// The root first, through the parent of `node`, once the walk has started.
    path: Option<Vec<Node<'tree>>>,
}

impl<'tree> Iterator for Ancestors<'tree> {
    type Item = Node<'tree>;

    fn next(&mut self) -> Option<Node<'tree>> {
        let node = self.node;
        self.path.get_or_insert_with(|| descent(node)).pop()
    }
}

/// The nodes `Node::parent` passes from the root down to `node`, in that order.
fn descent(node: Node<'_>) -> Vec<Node<'_>> {
    let root = root(node);
    if root.id() == node.id() {
        return Vec::new();
    }
    let mut path = vec![root];
    while let Some(next) = path[path.len() - 1].child_with_descendant(node) {
        if next.id() == node.id() {
            break;
        }
        path.push(next);
    }
    path
}

fn root(node: Node<'_>) -> Node<'_> {
    // SAFETY: `node` borrows its tree for the lifetime the root is returned with, and
    // `ts_tree_root_node` only reads that tree.
    unsafe { Node::from_raw(tree_sitter::ffi::ts_tree_root_node(node.into_raw().tree)) }
}
