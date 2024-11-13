use std::marker::PhantomData;

use rand::prelude::SliceRandom;
use rand::RngCore;

use crate::core::{OIndividual, OOError};
use crate::operators::{OBinaryComparisonOperator, OPreferredSolution};

/// A trait implementing methods to choose individuals from a population for reproduction.
pub trait OSelector {
    /// Select a number of individuals from the population equal to `number_of_winners`.
    ///
    /// # Arguments
    ///
    /// * `individuals`: The individuals.
    /// * `number_of_winners`: The number of winners to select.
    ///
    /// returns: `Result<Vec<Individual>, OError>`
    fn select(
        &self,
        individuals: &[OIndividual],
        number_of_winners: usize,
        rng: &mut dyn RngCore,
    ) -> Result<Vec<OIndividual>, OOError> {
        let mut winners: Vec<OIndividual> = Vec::new();
        for _ in 0..number_of_winners {
            winners.push(self.select_fit_individual(individuals, rng)?);
        }
        Ok(winners)
    }

    /// Select the fittest individual from the population.
    ///
    /// # Arguments
    ///
    /// * `individuals`: The list of individuals.
    /// * `rng`: The random number generator. Set this to `None`, if you are not using a seed.
    ///
    ///
    /// returns: `Result<Individual, OError>`
    fn select_fit_individual(
        &self,
        individuals: &[OIndividual],
        rng: &mut dyn RngCore,
    ) -> Result<OIndividual, OOError>;
}

/// Tournament selection method between multiple competitors for choosing individuals from a
/// population for reproduction. `number_of_competitors` individuals are randomly selected from the
/// population, then the most fit becomes a parent based on the provided `fitness` function.
/// More tournaments may be run to select more individuals.
pub struct OTournamentSelector<Operator: OBinaryComparisonOperator> {
    /// The number of competitors in each tournament. For example, 2 to run a binary tournament.
    number_of_competitors: usize,
    /// The function to use to assess the fitness and determine which individual wins a tournament.
    _fitness_function: PhantomData<Operator>,
}

impl<Operator: OBinaryComparisonOperator> OTournamentSelector<Operator> {
    /// Create a new tournament.
    ///
    /// # Arguments
    ///
    /// * `number_of_competitors`: The number of competitors in the tournament. Default to 2
    ///    individuals.
    ///
    /// returns: `TournamentSelector`
    pub fn new(number_of_competitors: usize) -> Self {
        Self {
            _fitness_function: PhantomData::<Operator>,
            number_of_competitors,
        }
    }
}

impl<Operator: OBinaryComparisonOperator> OSelector for OTournamentSelector<Operator> {
    /// Select the fittest individual from the population.
    ///
    /// # Arguments
    ///
    /// * `individuals`:The individuals with the solutions.
    /// * `rng`: The random number generator. Set this to `None`, if you are not using a seed.
    ///
    /// returns: `Result<Individual, OError>`
    fn select_fit_individual(
        &self,
        individuals: &[OIndividual],
        rng: &mut dyn RngCore,
    ) -> Result<OIndividual, OOError> {
        // let population = population.lock().unwrap();
        if individuals.is_empty() {
            return Err(OOError::SelectorOperator(
                "BinaryComparisonOperator".to_string(),
                "The population is empty and no individual can be selected".to_string(),
            ));
        }
        if individuals.len() < self.number_of_competitors {
            return Err(OOError::SelectorOperator(
                "BinaryComparisonOperator".to_string(),
                format!("The population size ({}) is smaller than the number of competitors needed in the tournament ({})", individuals.len(), self.number_of_competitors))
            );
        }
        let mut winner = individuals.choose(rng).unwrap();

        for _ in 0..self.number_of_competitors {
            let potential_winner = individuals.choose(rng).unwrap();
            let preferred_sol = Operator::compare(winner, potential_winner)?;
            if preferred_sol == OPreferredSolution::Second {
                winner = potential_winner;
            } else if preferred_sol == OPreferredSolution::MutuallyPreferred {
                // randomly select winner
                winner = [winner, potential_winner].choose(rng).unwrap();
            }
        }

        Ok(winner.clone())
    }
}
