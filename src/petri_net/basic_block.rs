use pnml;
use pnml::{NodeRef, PageRef, PetriNet, Result};
use rustc::mir;
use std::clone::Clone;

//    .-----.
// .-( start )------------------------------------------.
// |  '-----'            BasicBlock                     |
// |     |                                              |
// |     v                .-----------------------.     |
// |   .---.            .-----.  Statements       |     |
// |   |   |---------->( start )                  |     |
// |   '---'            '-----'                   |     |
// |     |                |                       |     |
// |     v                |                       |     |
// | .-------.            |                       |     |
// |( working )           |                       |     |
// | '-------'            |                       |     |
// |     |                |                       |     |
// |     v                |                       |     |
// |   .---.             .---.                    |     |
// |   |   |<-----------( end )                   |     |
// |   '---'             '---'                    |     |
// |     |                '-----------------------'     |
// |     v                                              |
// | .------.                                           |
// |( choice )----------------------------------.       |
// | '------'        |       |       |          |       |
// |     -.          |       |       |          |       |
// |.-----|----------|-------|-------|----------|-----. |
// ||     |          |  Terminator   |          |     | |
// ||     |          |       |       |          |     | |
// ||     v          v       v       v          v     | |
// ||  .-----.      .-.     .-.     .-.      .-----.  | |
// ''-( end_1 )----(   )---(   )---(   )----( end_N )-'-'
//     '-----'      '-'     '-'     '-'      '-----'
#[derive(Debug)]
pub struct BasicBlock {
    page: pnml::PageRef,
    start_place: NodeRef,
    end_place: NodeRef,
    pub statements: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct Statement {
    start_place: NodeRef,
    end_place: NodeRef,
}

impl BasicBlock {
    pub fn new<'net>(
        net: &'net mut PetriNet,
        page: &PageRef,
        start_place: &NodeRef,
    ) -> Result<Self> {
        let page = page.clone();
        let start_place = start_place.clone();
        let end_place = net.add_place(&page)?;
        // TODO: add flow
        let statements = Vec::new();
        //TODO: add statements
        Ok(BasicBlock {
            page,
            start_place,
            end_place,
            statements,
        })
    }
    pub fn add_statement<'net>(&mut self, net: &'net mut PetriNet) -> Result<()> {
        let start_place = {
            if let Some(statement) = self.statements.last() {
                statement.start_place().clone()
            } else {
                net.add_place(&self.page)?
            }
        };
        self.statements
            .push(Statement::new(net, &self.page, &start_place)?);
        Ok(())
    }
    pub fn start_place(&self) -> &NodeRef {
        &self.start_place
    }
    pub fn end_place(&self) -> &NodeRef {
        &self.end_place
    }
}

impl Statement {
    pub fn new<'net>(
        net: &'net mut PetriNet,
        page: &PageRef,
        start_place: &NodeRef,
    ) -> Result<Self> {
        let end_place = net.add_place(page)?;
        Ok(Statement {
            start_place: start_place.clone(),
            end_place,
        })
    }
    pub fn start_place(&self) -> &NodeRef {
        &self.start_place
    }
    pub fn end_place(&self) -> &NodeRef {
        &self.end_place
    }
}
