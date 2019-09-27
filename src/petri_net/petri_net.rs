use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

#[derive(Eq, PartialEq, Clone)]
pub struct PetriNet<PlaceA, TransitionA>
where
    PlaceA: PartialEq + Clone,
    TransitionA: PartialEq + Clone,
{
    pub(super) places: Vec<P<PlaceA>>,
    pub(super) transitions: Vec<T<TransitionA>>,
    pub(super) flow: Flow,
    pub(super) initial_marking: Marking,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub struct Place<'net, PlaceA, TransitionA>
where
    PlaceA: PartialEq + Clone,
    TransitionA: PartialEq + Clone,
{
    pub(super) index: usize,
    pub(super) net: &'net PetriNet<PlaceA, TransitionA>,
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub struct Transition<'net, PlaceA, TransitionA>
where
    PlaceA: PartialEq + Clone,
    TransitionA: PartialEq + Clone,
{
    pub(super) index: usize,
    pub(super) net: &'net PetriNet<PlaceA, TransitionA>,
}

pub enum Node<'net, PlaceAnnotation, TransitionAnnotation>
where
    PlaceAnnotation: PartialEq + Clone,
    TransitionAnnotation: PartialEq + Clone,
{
    Place(Place<'net, PlaceAnnotation, TransitionAnnotation>),
    Transition(Transition<'net, PlaceAnnotation, TransitionAnnotation>),
}

pub type Tokens = u64;

pub enum PetriError {
    BipartitionViolation,
    PlaceNotFound,
    TransitionNotFound,
    IoError(std::io::Error),
}

////////////////////////////////
// internal data representation
type Marking = Vec<Tokens>;

#[derive(Hash, Eq, PartialEq, Clone)]
pub(super) struct P<A> {
    pub(super) annotation: A,
}
#[derive(Hash, Eq, PartialEq, Clone)]
pub(super) struct T<A> {
    pub(super) annotation: A,
}
#[derive(Eq, PartialEq, Clone)]
pub struct Flow {
    /// arcs from places to transitions
    pub(super) pt: Vec<HashSet<usize>>,
    /// arcs from transitions to places
    pub(super) tp: Vec<HashSet<usize>>,
}
// end internal data
//////////////////////////////////

impl<'net, PA, TA> PetriNet<PA, TA>
where
    PA: PartialEq + Clone,
    TA: PartialEq + Clone,
{
    pub fn new() -> Self {
        PetriNet {
            places: Vec::new(),
            transitions: Vec::new(),
            flow: Flow {
                pt: Vec::new(),
                tp: Vec::new(),
            },
            initial_marking: Vec::new(),
        }
    }

    pub fn places(&'net self) -> Vec<Place<'net, PA, TA>> {
        Vec::from_iter(self.places.iter().map(|p| self.p_to_place(p).unwrap()))
    }

    pub fn transitions(&'net self) -> Vec<Transition<'net, PA, TA>> {
        Vec::from_iter(
            self.transitions
                .iter()
                .map(|t| self.t_to_transition(t).unwrap()),
        )
    }

    pub fn add_place(
        &'net mut self,
        annotation: PA,
        initial_tokens: Tokens,
    ) -> Place<'net, PA, TA> {
        self.places.push(P { annotation });
        self.flow.pt.push(HashSet::new());
        self.initial_marking.push(initial_tokens);
        self.assert_invariants();
        self.p_to_place(self.places.last().unwrap()).unwrap() //cannot fail
    }

    pub fn add_transition(&'net mut self, annotation: TA) -> Transition<'net, PA, TA> {
        // add the transition to the places set
        self.transitions.push(T { annotation });
        // add an entry in the flow
        self.flow.tp.push(HashSet::new());
        self.assert_invariants();
        self.t_to_transition(self.transitions.last().unwrap())
            .unwrap() //cannot fail
    }

    fn add_pt_edge(
        &mut self,
        from_p: &Place<'net, PA, TA>,
        to_t: &Transition<'net, PA, TA>,
    ) -> Result<(), PetriError> {
        Self::place_to_p(from_p)?;
        Self::transition_to_t(to_t)?;
        let tSet = self
            .flow
            .pt
            .get_mut(from_p.index)
            .expect("flow Vector should always be synced with the place vector");
        tSet.insert(to_t.index);
        self.assert_invariants();
        Ok(())
    }

    pub(super) fn add_tp_edge(
        &mut self,
        from_t: &Transition<'net, PA, TA>,
        to_p: &Place<'net, PA, TA>,
    ) -> Result<(), PetriError> {
        Self::transition_to_t(from_t)?;
        Self::place_to_p(to_p)?;
        let pSet = self
            .flow
            .tp
            .get_mut(from_t.index)
            .expect("flow Vector should always be synced with the place vector");
        pSet.insert(to_p.index);
        self.assert_invariants();
        Ok(())
    }

    pub fn add_edge(
        &mut self,
        from: Node<'net, PA, TA>,
        to: Node<'net, PA, TA>,
    ) -> Result<(), PetriError> {
        Ok(())
    }

    pub(super) fn p_to_place(&'net self, place: &P<PA>) -> Result<Place<'net, PA, TA>, PetriError> {
        let i = self
            .places
            .iter()
            .position(|elem| elem == place)
            .ok_or(PetriError::PlaceNotFound)?;
        Ok(Place {
            index: i,
            net: &self,
        })
    }

    pub(super) fn place_to_p(place: &'net Place<'net, PA, TA>) -> Result<&P<PA>, PetriError> {
        place
            .net
            .places
            .get(place.index)
            .ok_or(PetriError::PlaceNotFound)
    }
    pub(super) fn t_to_transition(
        &'net self,
        transition: &T<TA>,
    ) -> Result<Transition<'net, PA, TA>, PetriError> {
        let i = self
            .transitions
            .iter()
            .position(|elem| elem == transition)
            .ok_or(PetriError::TransitionNotFound)?;
        Ok(Transition {
            index: i,
            net: &self,
        })
    }

    pub(super) fn transition_to_t(
        transition: &'net Transition<'net, PA, TA>,
    ) -> Result<&T<TA>, PetriError> {
        transition
            .net
            .transitions
            .get(transition.index)
            .ok_or(PetriError::TransitionNotFound)
    }
    pub(super) fn assert_invariants(&self) {
        assert_eq!(self.places.len(), self.initial_marking.len());
        assert_eq!(self.places.len(), self.flow.pt.len());
        assert_eq!(self.transitions.len(), self.flow.tp.len());
    }
}

impl<'net, PA, TA> Transition<'net, PA, TA>
where
    PA: PartialEq + Clone,
    TA: PartialEq + Clone,
{
    pub fn annotation(&self) -> Result<&TA, PetriError> {
        Ok(&PetriNet::transition_to_t(self)?.annotation)
    }
}

impl<'net, PA, TA> Place<'net, PA, TA>
where
    PA: PartialEq + Copy + Clone,
    TA: PartialEq + Copy + Clone,
{
    pub fn annotation(&self) -> Result<&PA, PetriError> {
        Ok(&PetriNet::place_to_p(self)?.annotation)
    }
}
