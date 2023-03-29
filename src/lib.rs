/// Arithmetic progression sum circuit.
///
/// This circuit computes the sum an arithmetic progression with a given `step` and `count`.
///
/// For example, for step `1` and count `5`, with the first item as `1`, the circuit
/// computes 1 + 2 + 3 + 4 + 5 = 15.

use halo2_proofs::{arithmetic::FieldExt, circuit::*, plonk::*, poly::Rotation};
use std::marker::PhantomData;

#[derive(Clone, Debug)]
struct ApSumConfig {
    // [a_n, sum_n]
    advice: [Column<Advice>; 2],
    selector: Selector,
    instance: Column<Instance>,
}

struct ApSumChip<F, const STEP: u128, const COUNT: usize> {
    config: ApSumConfig,
    _marker: PhantomData<F>,
}

impl<F: FieldExt, const STEP: u128, const COUNT: usize> ApSumChip<F, STEP, COUNT> {
    fn construct(config: ApSumConfig) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 2],
        instance: Column<Instance>,
    ) -> ApSumConfig {
        let selector = meta.selector();

        meta.enable_equality(advice[0]);
        meta.enable_equality(advice[1]);
        meta.enable_equality(instance);

        // |  advice[0]   |    advice[1]   | selector
        // -------------------------------------------
        // |     a_0      |     sum_0      |
        // |     a_1      |     sum_1      |    s
        // |     a_2      |     sum_2      |    s
        // |     ...      |     ...        |    s
        meta.create_gate("step and sum", |meta| {
            let a = meta.query_advice(advice[0], Rotation::cur());
            let sum = meta.query_advice(advice[1], Rotation::cur());
            let prev_a = meta.query_advice(advice[0], Rotation::prev());
            let prev_sum = meta.query_advice(advice[1], Rotation::prev());
            let s = meta.query_selector(selector);
            vec![
                // sum == a + prev_sum
                s.clone() * (a.clone() + prev_sum - sum),
                // a == prev_a + STEP
                s * (a - prev_a - Expression::Constant(F::from_u128(STEP))),
            ]
        });

        ApSumConfig {
            advice,
            selector,
            instance,
        }
    }

    fn assign(&self, mut layouter: impl Layouter<F>) -> Result<AssignedCell<F, F>, Error> {
        layouter.assign_region(
            || "AP sum table",
            |mut region| {
                let a_column = self.config.advice[0];
                let sum_column = self.config.advice[1];

                // Copy first instance into both a_0 and sum_0. No selector needed for first row.
                let mut a_cell = region.assign_advice_from_instance(
                    || "a",
                    self.config.instance,
                    0,
                    a_column,
                    0,
                )?;
                let mut sum_cell = region.assign_advice_from_instance(
                    || "sum",
                    self.config.instance,
                    0,
                    sum_column,
                    0,
                )?;

                for row in 1..COUNT {
                    self.config.selector.enable(&mut region, row)?;

                    let new_a_val = a_cell
                        .value()
                        .and_then(|a| Value::known(*a + F::from_u128(STEP)));
                    a_cell = region.assign_advice(|| "a", a_column, row, || new_a_val)?;

                    let new_sum = sum_cell
                        .value()
                        .and_then(|sum| new_a_val.map(|new_a| new_a + sum));
                    sum_cell = region.assign_advice(|| "sum", sum_column, row, || new_sum)?;
                }

                Ok(sum_cell)
            },
        )
    }

    fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        cell: &AssignedCell<F, F>,
        row: usize,
    ) -> Result<(), Error> {
        layouter.constrain_instance(cell.cell(), self.config.instance, row)
    }
}

#[derive(Default)]
struct ApSumCircuit<const STEP: u128, const COUNT: usize>;

impl<F: FieldExt, const STEP: u128, const COUNT: usize> Circuit<F> for ApSumCircuit<STEP, COUNT> {
    type Config = ApSumConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = [meta.advice_column(), meta.advice_column()];
        let instance = meta.instance_column();

        ApSumChip::<_, STEP, COUNT>::configure(meta, advice, instance)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let chip = ApSumChip::<_, STEP, COUNT>::construct(config);
        let sum_cell = chip.assign(layouter.namespace(|| "AP sum table"))?;
        chip.expose_public(layouter.namespace(|| "output"), &sum_cell, 1)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp};

    #[test]
    fn ap_sum_step_one_count_five_works() {
        let k = 5;
        let circuit = ApSumCircuit::<1, 5>;
        // 1 + 2 + 3 + 4 + 5 = 15
        let prover = MockProver::run(k, &circuit, vec![vec![Fp::from(1), Fp::from(15)]]).unwrap();
        prover.assert_satisfied();

        // circuit layout
        #[cfg(feature = "dev-graph")]
        {
            use plotters::prelude::*;

            let mut root = BitMapBackend::new("fib-layout.png", (1024, 3096)).into_drawing_area();
            root.fill(&WHITE).unwrap();
            root = root.titled("Fib 2 layout", ("sans-serif", 60)).unwrap();
            halo2_proofs::dev::CircuitLayout::default()
                .render::<Fp, _, _>(k, &circuit, &root)
                .unwrap();
        }
    }

    #[test]
    fn ap_sum_step_three_count_four_works() {
        let k = 4;
        let circuit = ApSumCircuit::<3, 4>;
        // 1 + 4 + 7 + 10 = 22
        let prover = MockProver::run(k, &circuit, vec![vec![Fp::from(1), Fp::from(22)]]).unwrap();
        prover.assert_satisfied();
    }
}
