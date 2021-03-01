//! A solver that uses [highs](https://docs.rs/highs), a parallel C++ solver.

use highs::HighsModelStatus;

use crate::{Constraint, IntoAffineExpression, Variable};
use crate::solvers::{ObjectiveDirection, ResolutionError, Solution, SolverModel};
use crate::variable::{UnsolvedProblem, VariableDefinition};

/// The [minilp](https://docs.rs/minilp) solver,
/// to be used with [UnsolvedProblem::using].
pub fn highs(to_solve: UnsolvedProblem) -> HighsProblem {
    let mut highs_problem = highs::RowProblem::default();
    let mut model = highs::Model::new();
    model.set_sense(match to_solve.direction {
        ObjectiveDirection::Maximisation => highs::Sense::Maximise,
        ObjectiveDirection::Minimisation => highs::Sense::Minimise,
    });
    let mut columns = Vec::with_capacity(to_solve.variables.len());
    for (var, &VariableDefinition { min, max, .. }) in to_solve.variables.iter_variables_with_def() {
        let &col_factor = to_solve.objective.linear.coefficients.get(&var).unwrap_or(&0.);
        let col = highs_problem.add_column(col_factor, min..max);
        columns.push(col);
    }
    HighsProblem { model, highs_problem, columns }
}

/// A minilp model
#[derive(Debug)]
pub struct HighsProblem {
    model: highs::Model,
    highs_problem: highs::RowProblem,
    columns: Vec<highs::Col>,
}

impl HighsProblem {
    /// Get a highs model for this problem
    pub fn into_inner(mut self) -> highs::Model {
        self.model.set_problem(self.highs_problem);
        self.model
    }
}

impl SolverModel for HighsProblem {
    type Solution = HighsSolution;
    type Error = ResolutionError;

    fn with(mut self, constraint: Constraint) -> Self {
        let upper_bound = -constraint.expression.constant();
        let columns = &self.columns;
        let factors = constraint.expression.linear_coefficients().into_iter()
            .map(|(variable, factor)| {
                (columns[variable.index()], factor)
            });
        if constraint.is_equality {
            self.highs_problem.add_row(upper_bound..=upper_bound, factors);
        } else {
            self.highs_problem.add_row(..=upper_bound, factors);
        }
        self
    }

    fn solve(mut self) -> Result<Self::Solution, Self::Error> {
        self.model.set_problem(self.highs_problem);
        let solved = self.model.solve();
        match solved.status() {
            HighsModelStatus::NotSet => Err(ResolutionError::Other("NotSet")),
            HighsModelStatus::LoadError => Err(ResolutionError::Other("LoadError")),
            HighsModelStatus::ModelError => Err(ResolutionError::Other("ModelError")),
            HighsModelStatus::PresolveError => Err(ResolutionError::Other("PresolveError")),
            HighsModelStatus::SolveError => Err(ResolutionError::Other("SolveError")),
            HighsModelStatus::PostsolveError => Err(ResolutionError::Other("PostsolveError")),
            HighsModelStatus::ModelEmpty => Err(ResolutionError::Other("ModelEmpty")),
            HighsModelStatus::PrimalInfeasible => Err(ResolutionError::Infeasible),
            HighsModelStatus::PrimalUnbounded => Err(ResolutionError::Unbounded),
            _ok_status => {
                Ok(HighsSolution {
                    solution: solved.get_solution()
                })
            }
        }
    }
}

/// The solution to a highs problem
#[derive(Debug)]
pub struct HighsSolution {
    solution: highs::Solution,
}

impl HighsSolution {
    /// Returns the highs solution object. You can use it to fetch dual values
    pub fn into_inner(self) -> highs::Solution {
        self.solution
    }
}

impl Solution for HighsSolution {
    fn value(&self, variable: Variable) -> f64 {
        self.solution.columns()[variable.index()]
    }
}