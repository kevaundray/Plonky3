//! The Poseidon2 permutation.
//!
//! This implementation was based upon the following resources:
//! - https://github.com/HorizenLabs/poseidon2/blob/main/plain_implementations/src/poseidon2/poseidon2.rs
//! - https://eprint.iacr.org/2023/323.pdf

#![no_std]

extern crate alloc;

mod constants;
mod diffusion;
mod matrix;
mod round_numbers;
use alloc::vec::Vec;

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
mod x86_64_avx2;
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
pub use x86_64_avx2::*;

pub use constants::*;
pub use diffusion::*;
pub use matrix::*;
use p3_field::{AbstractField, Field, PrimeField, PrimeField64};
use p3_symmetric::{CryptographicPermutation, Permutation};
use rand::distributions::{Distribution, Standard};
use rand::Rng;
pub use round_numbers::poseidon2_round_numbers_128;

const SUPPORTED_WIDTHS: [usize; 8] = [2, 3, 4, 8, 12, 16, 20, 24];

/// The Basic Poseidon2 permutation.
#[derive(Clone, Debug)]
pub struct Poseidon2<
    F,
    MdsLightLayer,
    DiffusionLayer,
    PackedConstants,
    const WIDTH: usize,
    const D: u64,
> where
    F: Field,
    PackedConstants: Poseidon2PackedTypesAndConstants<F, WIDTH>,
{
    /// The external round constants.
    external_constants: [Vec<[F; WIDTH]>; 2],

    /// The external round constants.
    external_packed_constants: [Vec<PackedConstants::ExternalConstantsType>; 2],

    /// The linear layer used in External Rounds. Should be either MDS or a
    /// circulant matrix based off an MDS matrix of size 4.
    external_layer: MdsLightLayer,

    /// The internal round constants.
    internal_constants: Vec<F>,

    /// The internal round constants.
    internal_packed_constants: Vec<PackedConstants::InternalConstantsType>,

    /// The linear layer used in internal rounds (only needs diffusion property, not MDS).
    internal_layer: DiffusionLayer,
}

impl<F, MdsLightLayer, DiffusionLayer, PackedConstants, const WIDTH: usize, const D: u64>
    Poseidon2<F, MdsLightLayer, DiffusionLayer, PackedConstants, WIDTH, D>
where
    F: PrimeField,
    PackedConstants: Poseidon2PackedTypesAndConstants<F, WIDTH>,
{
    /// Create a new Poseidon2 configuration.
    pub fn new(
        external_constants: [Vec<[F; WIDTH]>; 2],
        external_layer: MdsLightLayer,
        internal_constants: Vec<F>,
        internal_layer: DiffusionLayer,
    ) -> Self {
        assert!(SUPPORTED_WIDTHS.contains(&WIDTH));

        let external_packed_constants: [Vec<_>; 2] =
            external_constants.clone().map(|constant_list| {
                constant_list
                    .iter()
                    .map(|constants| PackedConstants::manipulate_external_constants(constants))
                    .collect()
            });
        let internal_packed_constants: Vec<_> = internal_constants
            .iter()
            .map(|val| PackedConstants::manipulate_internal_constants(val))
            .collect();

        Self {
            external_constants,
            external_packed_constants,
            external_layer,
            internal_constants,
            internal_packed_constants,
            internal_layer,
        }
    }

    /// Create a new Poseidon2 configuration with random parameters.
    pub fn new_from_rng<R: Rng>(
        rounds_f: usize,
        external_layer: MdsLightLayer,
        rounds_p: usize,
        internal_layer: DiffusionLayer,
        rng: &mut R,
    ) -> Self
    where
        Standard: Distribution<F> + Distribution<[F; WIDTH]>,
    {
        let half_f = rounds_f / 2;
        let init_external_constants = rng.sample_iter(Standard).take(half_f).collect();
        let final_external_constants = rng.sample_iter(Standard).take(half_f).collect();
        let external_constants = [init_external_constants, final_external_constants];
        let internal_constants = rng.sample_iter(Standard).take(rounds_p).collect::<Vec<F>>();

        Self::new(
            external_constants,
            external_layer,
            internal_constants,
            internal_layer,
        )
    }
}

impl<F, MdsLightLayer, DiffusionLayer, PackedConstants, const WIDTH: usize, const D: u64>
    Poseidon2<F, MdsLightLayer, DiffusionLayer, PackedConstants, WIDTH, D>
where
    F: PrimeField64,
    PackedConstants: Poseidon2PackedTypesAndConstants<F, WIDTH>,
{
    /// Create a new Poseidon2 configuration with 128 bit security and random rounds constants.
    pub fn new_from_rng_128<R: Rng>(
        external_layer: MdsLightLayer,
        internal_layer: DiffusionLayer,
        rng: &mut R,
    ) -> Self
    where
        Standard: Distribution<F> + Distribution<[F; WIDTH]>,
    {
        let (rounds_f, rounds_p) = poseidon2_round_numbers_128::<F>(WIDTH, D);
        let half_f = rounds_f / 2;
        let init_external_constants = rng.sample_iter(Standard).take(half_f).collect();
        let final_external_constants = rng.sample_iter(Standard).take(half_f).collect();

        let external_constants = [init_external_constants, final_external_constants];
        let internal_constants = rng.sample_iter(Standard).take(rounds_p).collect::<Vec<F>>();

        Self::new(
            external_constants,
            external_layer,
            internal_constants,
            internal_layer,
        )
    }
}

impl<AF, MdsLightLayer, DiffusionLayer, PackedConstants, const WIDTH: usize, const D: u64>
    Permutation<[AF; WIDTH]>
    for Poseidon2<AF::F, MdsLightLayer, DiffusionLayer, PackedConstants, WIDTH, D>
where
    AF: AbstractField,
    AF::F: PrimeField,
    PackedConstants: Poseidon2PackedTypesAndConstants<AF::F, WIDTH>,
    MdsLightLayer: ExternalLayer<AF, PackedConstants, WIDTH, D>,
    DiffusionLayer:
        InternalLayer<AF, PackedConstants, WIDTH, D, InternalState = MdsLightLayer::InternalState>,
{
    fn permute(&self, state: [AF; WIDTH]) -> [AF; WIDTH] {
        let mut internal_state = self.external_layer.to_internal_rep(state);

        for sub_state in internal_state.as_mut() {
            // The first half of the external rounds.
            self.external_layer.permute_state_initial(
                sub_state,
                &self.external_constants[0],
                &self.external_packed_constants[0],
            );

            // The internal rounds.
            self.internal_layer.permute_state(
                sub_state,
                &self.internal_constants,
                &self.internal_packed_constants,
            );

            // The second half of the external rounds.
            self.external_layer.permute_state_final(
                sub_state,
                &self.external_constants[1],
                &self.external_packed_constants[1],
            );
        }

        self.external_layer.to_output_rep(internal_state)
    }

    fn permute_mut(&self, state: &mut [AF; WIDTH]) {
        *state = self.permute((*state).clone())
    }
}

impl<AF, MdsLightLayer, DiffusionLayer, PackedConstants, const WIDTH: usize, const D: u64>
    CryptographicPermutation<[AF; WIDTH]>
    for Poseidon2<AF::F, MdsLightLayer, DiffusionLayer, PackedConstants, WIDTH, D>
where
    AF: AbstractField,
    AF::F: PrimeField,
    PackedConstants: Poseidon2PackedTypesAndConstants<AF::F, WIDTH>,
    MdsLightLayer: ExternalLayer<AF, PackedConstants, WIDTH, D>,
    DiffusionLayer:
        InternalLayer<AF, PackedConstants, WIDTH, D, InternalState = MdsLightLayer::InternalState>,
{
}
