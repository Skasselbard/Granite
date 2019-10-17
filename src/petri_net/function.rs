use crate::petri_net::basic_block::BasicBlock;
use pnml;
use pnml::{NodeRef, PageRef, PetriNet, Result};
use rustc::hir::def_id::DefId;
use rustc::mir;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Function<'mir> {
    pub mir_id: DefId,
    pub mir_body: &'mir mir::Body<'mir>,
    basic_blocks: HashMap<mir::BasicBlock, BasicBlock>,
    active_block: Option<mir::BasicBlock>,
    page: PageRef,
    start_place: NodeRef,
    end_place: NodeRef,
}

impl<'mir> Function<'mir> {
    pub fn new<'net>(
        mir_id: DefId,
        mir_body: &'mir mir::Body<'mir>,
        net: &'net mut PetriNet,
        start_place: NodeRef,
        name: &str,
    ) -> Result<Self> {
        let page = net.add_page(Some(name));
        let end_place = net.add_place(&page)?;
        Ok(Function {
            mir_id,
            mir_body,
            basic_blocks: HashMap::new(),
            active_block: None,
            page,
            start_place,
            end_place,
        })
    }

    pub fn goto<'net>(&mut self, net: &'net mut PetriNet, to: &mir::BasicBlock) -> Result<()> {
        let page = self.page.clone();
        let t = net.add_transition(&page)?;
        let active_block = self
            .basic_blocks
            .get(&self.active_block.expect("activeBlock was not initialized"))
            .expect("unable to find active basic block in function");
        net.add_arc(&self.page, active_block.end_place(), &t)?;
        let to = match self.basic_blocks.get(to) {
            Some(block) => block.start_place(),
            None => self.add_basic_block(net, to)?.start_place(),
        };
        net.add_arc(&page, &t, to)?;
        Ok(())
    }

    pub fn activate_block(&mut self, block: &mir::BasicBlock) {
        self.active_block = Some(*block);
    }

    fn add_basic_block<'net>(
        &mut self,
        net: &'net mut PetriNet,
        block: &mir::BasicBlock,
    ) -> Result<&BasicBlock> {
        let start_place = net.add_place(&self.page)?;
        let bb = BasicBlock::new(net, &self.page, &start_place)?;
        self.basic_blocks
            .insert(*block, bb)
            .expect_none("this should not happen");
        Ok(self
            .basic_blocks
            .get(block)
            .expect("this also should not happen"))
    }
}
