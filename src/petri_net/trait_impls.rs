use super::petri_net::{Node, PetriError, Place, Transition};
use std::error::Error;
use std::{fmt, io};

impl Error for PetriError {
    fn description(&self) -> &str {
        match self {
            PetriError::BipartitionViolation => "Edges cannot lead to identical Node types. They are only allowed from places to transitions or vice versa",
            PetriError::PlaceNotFound => "There is no corresponding place in the internal representation",
            PetriError::TransitionNotFound => "There is no corresponding transition in the internal representation",
            PetriError::IoError(error) => error.description()
        }
    }
}

impl fmt::Display for PetriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl fmt::Debug for PetriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl From<io::Error> for PetriError {
    fn from(error: io::Error) -> Self {
        PetriError::IoError(error)
    }
}

impl<'net, PA, TA> From<Place<'net, PA, TA>> for Node<'net, PA, TA>
where
    PA: PartialEq + Clone,
    TA: PartialEq + Clone,
{
    fn from(place: Place<'net, PA, TA>) -> Self {
        Node::Place(place)
    }
}

impl<'net, PA, TA> From<Transition<'net, PA, TA>> for Node<'net, PA, TA>
where
    PA: PartialEq + Clone,
    TA: PartialEq + Clone,
{
    fn from(transition: Transition<'net, PA, TA>) -> Self {
        Node::Transition(transition)
    }
}
