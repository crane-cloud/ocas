use std::error::Error;

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::core::{OEvaluationResult, OEvaluator, OIndividual};

/// Get the random number generator. If no seed is provided, this randomly generated.
///
/// # Arguments
///
/// * `seed`: The optional seed number.
///
/// returns: `Box<dyn RngCore>`
pub(crate) fn get_rng(seed: Option<u64>) -> Box<dyn RngCore> {
    let rng = match seed {
        None => ChaCha8Rng::from_seed(Default::default()),
        Some(s) => ChaCha8Rng::seed_from_u64(s),
    };
    Box::new(rng)
}

/// Return a dummy evaluator.
///
/// return `Box<dyn OEvaluator>`
pub fn dummy_evaluator() -> Box<dyn OEvaluator> {
    #[derive(Debug)]
    struct UserEvaluator;

    impl OEvaluator for UserEvaluator {
        fn evaluate(&self, _: &OIndividual) -> Result<OEvaluationResult, Box<dyn Error>> {
            unimplemented!("The evaluation method is not implemented");
        }
    }

    Box::new(UserEvaluator)
}
