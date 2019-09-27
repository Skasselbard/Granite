use crate::petri_net::{self};
use rustc::mir;
use std::clone::Clone;

// Make the types readable
type TransitionAnnotation = ();
type PetriNet = petri_net::PetriNet<PlaceAnnotation, TransitionAnnotation>;
type Place<'mir> = petri_net::Place<'mir, PlaceAnnotation, TransitionAnnotation>;
type Transition<'mir> = petri_net::Transition<'mir, PlaceAnnotation, TransitionAnnotation>;

#[derive(Hash, Eq, PartialEq, Copy, Clone)]
pub enum PlaceAnnotation {
    Start,
    End,
}

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
#[derive(Clone)]
pub struct BasicBlock<'mir> {
    pub mir_bb: &'mir mir::BasicBlockData<'mir>,
    pub net: PetriNet,

    pub statements: Vec<Statement>,
    phantom: (), // enforce constructor build
}

#[derive(Clone)]
pub struct Statement {}

impl<'mir> BasicBlock<'mir> {
    pub fn new(mir_bb: &'mir mir::BasicBlockData<'mir>) -> Self {
        let mut net = PetriNet::new();
        // TODO: add places
        net.add_place(PlaceAnnotation::Start, 0);
        net.add_place(PlaceAnnotation::End, 0);
        // TODO: add flow
        let statements = Vec::new();
        //TODO: add statements
        BasicBlock {
            mir_bb,
            net,
            statements,
            phantom: (),
        }
    }
}
