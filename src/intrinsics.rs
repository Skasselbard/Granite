use petri_to_star::{NodeRef, PetriNet, Result};

pub(crate) fn generic_foreign(
    net: &mut PetriNet,
    arg_nodes: &Vec<NodeRef>,
    source_node: NodeRef,
    destination_node: NodeRef, // local var that holds the return value
    destination_block_start: NodeRef, // start of bb to continue
    cleanup_node: Option<NodeRef>, // start of fail case bb
) -> Result<()> {
    //flow
    let t = net.add_transition();
    net.add_arc(source_node, t)?;
    net.add_arc(t, destination_block_start)?;
    if let Some(node) = cleanup_node {
        net.add_arc(t, node)?;
    }
    //vars
    net.add_arc(destination_node, t)?;
    net.add_arc(t, destination_node)?;
    for node in arg_nodes {
        net.add_arc(*node, t)?;
        net.add_arc(t, *node)?;
    }
    Ok(())
}
