use crate::{
    state::*,
    settings::state::*
};
use futures_signals::{
    map_ref,
    signal::{Signal, SignalExt, Mutable},
    signal_vec::{SignalVec, SignalVecExt}
};

use std::rc::Rc;
use utils::prelude::*;
use shared::domain::jig::module::body::_groups::cards::Card;
use components::module::_groups::cards::lookup::Side;
use rand::{prelude::*, distributions::{Standard, Distribution}};

pub struct MainSettings {
    pub base: Rc<Base>,
    pub pairs: Rc<Vec<(Card, Card)>>,
}

    //pub pairs: MutableVec<(Card, Card)>,
impl MainSettings {
    pub fn new(base: Rc<Base>) -> Self {
        let settings = &base.extra.settings;

        let mut pairs = base.clone_pairs_raw();

        pairs.shuffle(&mut *base.extra.settings.rng.borrow_mut());

        Self {
            base,
            pairs: Rc::new(pairs),
        }
    }

    pub fn top_choices_signal(&self) -> impl Signal<Item = Vec<(Card, Side)>> {
        self.choices_signal(self.base.extra.settings.top_side.signal())
    }

    pub fn bottom_choices_signal(&self) -> impl Signal<Item = Vec<(Card, Side)>> {
        self.choices_signal(self.base.extra.settings.top_side.signal().map(|side| side.negate()))
    }

    fn choices_signal(&self, side_signal: impl Signal<Item = Side>) -> impl Signal<Item = Vec<(Card, Side)>> {
        let pairs = self.pairs.clone();

        let sig = map_ref! {
            let side = side_signal,
            let n_choices = self.base.extra.settings.n_choices.signal()
                => (*side, *n_choices)
        };
       
        sig
            .map(move |(side, n_choices)| {
                pairs
                    .iter()
                    .take(n_choices)
                    .map(|pair| {
                        let card = {
                            if side == Side::Left {
                                pair.0.clone()
                            } else {
                                pair.1.clone()
                            }
                        };

                        (card, side)
                    })
                    .collect::<Vec<(Card, Side)>>()
            })
    }


    pub fn get_random<T>(&self) -> T 
    where 
        Standard: Distribution<T>
    {
        self.base.extra.settings.rng.borrow_mut().gen::<T>()
    }
}
