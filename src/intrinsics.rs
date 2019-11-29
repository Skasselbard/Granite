use crate::petri_net::function::{Data, Function};
use pnml::{NodeRef, PNMLDocument, PageRef, PetriNet, Result};
use rustc::hir::def_id::DefId;
use rustc::mir::visit::Visitor;
use rustc::mir::visit::*;
use rustc::mir::{self, *};
use rustc::ty::{self, Ty, TyCtxt};

pub(crate) fn generic_foreign(
    net: &mut PetriNet,
    page: &PageRef,
    arg_nodes: &Vec<&NodeRef>,
    source_node: &NodeRef,
    destination_node: &NodeRef, // local var that holds the return value
    destination_block_start: &NodeRef, // start of bb to continue
    cleanup_node: Option<NodeRef>, // start of fail case bb
) -> Result<()> {
    //flow
    let t = net.add_transition(page)?;
    net.add_arc(page, source_node, &t)?;
    net.add_arc(page, &t, destination_block_start)?;
    if let Some(node) = cleanup_node {
        net.add_arc(page, &t, &node)?;
    }
    //vars
    net.add_arc(page, destination_node, &t)?;
    net.add_arc(page, &t, destination_node)?;
    for node in arg_nodes {
        net.add_arc(page, node, &t)?;
        net.add_arc(page, &t, node)?;
    }
    Ok(())
}
