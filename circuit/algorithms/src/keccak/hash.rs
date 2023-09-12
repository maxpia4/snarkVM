// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

impl<E: Environment, const TYPE: u8, const VARIANT: usize> Hash for Keccak<E, TYPE, VARIANT> {
    type Input = Boolean<E>;
    type Output = Vec<Boolean<E>>;

    /// Returns the Keccak hash of the given input as bits.
    #[inline]
    fn hash(&self, input: &[Self::Input]) -> Self::Output {
        // The bitrate `r`.
        let bitrate = (200 - (VARIANT / 4)) * 8;
        debug_assert!(bitrate < PERMUTATION_WIDTH, "The bitrate must be less than the permutation width");

        // Ensure the input is not empty.
        if input.is_empty() {
            E::halt("The input to the hash function must not be empty")
        }

        // The root state `s` is defined as `0^b`.
        let mut s = vec![Boolean::constant(false); PERMUTATION_WIDTH];

        // The padded blocks `P`.
        let padded_blocks = match TYPE {
            0 => Self::pad_keccak(input, bitrate),
            1 => Self::pad_sha3(input, bitrate),
            2.. => unreachable!("Invalid Keccak type"),
        };

        /* The first part of the sponge construction (the absorbing phase):
         *
         * for i = 0 to |P|_r − 1 do
         *   s = s ⊕ (P_i || 0^c) # Note: |P_i| + c == b, since |P_i| == r
         *   s = f(s)
         * end for
         */
        for block in padded_blocks {
            // s = s ⊕ (P_i || 0^c)
            for (j, bit) in block.into_iter().enumerate() {
                s[j] = &s[j] ^ &bit;
            }
            // s = f(s)
            s = Self::permutation_f::<PERMUTATION_WIDTH, NUM_ROUNDS>(s, &self.round_constants, &self.rotl);
        }

        /* The second part of the sponge construction (the squeezing phase):
         *
         * Z = s[0..r-1]
         * while |Z|_r < l do
         *   s = f(s)
         *   Z = Z || s[0..r-1]
         * end while
         * return Z[0..l-1]
         */
        // Z = s[0..r-1]
        let mut z = s[..bitrate].to_vec();
        // while |Z|_r < l do
        while z.len() < VARIANT {
            // s = f(s)
            s = Self::permutation_f::<PERMUTATION_WIDTH, NUM_ROUNDS>(s, &self.round_constants, &self.rotl);
            // Z = Z || s[0..r-1]
            z.extend(s.iter().take(bitrate).cloned());
        }
        // return Z[0..l-1]
        z.into_iter().take(VARIANT).collect()
    }
}

impl<E: Environment, const TYPE: u8, const VARIANT: usize> Keccak<E, TYPE, VARIANT> {
    /// In Keccak, `pad` is a multi-rate padding, defined as `pad(M) = M || 0x01 || 0x00…0x00 || 0x80`,
    /// where `M` is the input data, and `0x01 || 0x00…0x00 || 0x80` is the padding.
    /// The padding extends the input data to a multiple of the bitrate `r`, defined as `r = b - c`,
    /// where `b` is the width of the permutation, and `c` is the capacity.
    fn pad_keccak(input: &[Boolean<E>], bitrate: usize) -> Vec<Vec<Boolean<E>>> {
        debug_assert!(bitrate > 0, "The bitrate must be positive");

        // Resize the input to a multiple of 8.
        let mut padded_input = input.to_vec();
        padded_input.resize((input.len() + 7) / 8 * 8, Boolean::constant(false));

        // Step 1: Append the bit "1" to the message.
        padded_input.push(Boolean::constant(true));

        // Step 2: Append "0" bits until the length of the message is congruent to r-1 mod r.
        while (padded_input.len() % bitrate) != (bitrate - 1) {
            padded_input.push(Boolean::constant(false));
        }

        // Step 3: Append the bit "1" to the message.
        padded_input.push(Boolean::constant(true));

        // Construct the padded blocks.
        let mut result = Vec::new();
        for block in padded_input.chunks(bitrate) {
            result.push(block.to_vec());
        }
        result
    }

    /// In SHA-3, `pad` is a SHAKE, defined as `pad(M) = M || 0x06 || 0x00…0x00 || 0x80`,
    /// where `M` is the input data, and `0x06 || 0x00…0x00 || 0x80` is the padding.
    /// The padding extends the input data to a multiple of the bitrate `r`, defined as `r = b - c`,
    /// where `b` is the width of the permutation, and `c` is the capacity.
    fn pad_sha3(input: &[Boolean<E>], bitrate: usize) -> Vec<Vec<Boolean<E>>> {
        debug_assert!(bitrate > 1, "The bitrate must be greater than 1");

        // Resize the input to a multiple of 8.
        let mut padded_input = input.to_vec();
        padded_input.resize((input.len() + 7) / 8 * 8, Boolean::constant(false));

        // Step 1: Append the "0x06" byte to the message.
        padded_input.push(Boolean::constant(false));
        padded_input.push(Boolean::constant(true));
        padded_input.push(Boolean::constant(true));
        padded_input.push(Boolean::constant(false));

        // Step 2: Append "0" bits until the length of the message is congruent to r-1 mod r.
        while (padded_input.len() % bitrate) != (bitrate - 1) {
            padded_input.push(Boolean::constant(false));
        }

        // Step 3: Append the bit "1" to the message.
        padded_input.push(Boolean::constant(true));

        // Construct the padded blocks.
        let mut result = Vec::new();
        for block in padded_input.chunks(bitrate) {
            result.push(block.to_vec());
        }
        result
    }

    /// The permutation `f` is a function that takes a fixed-length input and produces a fixed-length output,
    /// defined as `f = Keccak-f[b]`, where `b := 25 * 2^l` is the width of the permutation,
    /// and `l` is the log width of the permutation.
    ///
    /// The round function `R` is applied `12 + 2l` times, where `l` is the log width of the permutation.
    fn permutation_f<const WIDTH: usize, const NUM_ROUNDS: usize>(
        input: Vec<Boolean<E>>,
        round_constants: &[U64<E>],
        rotl: &[usize],
    ) -> Vec<Boolean<E>> {
        debug_assert_eq!(input.len(), WIDTH, "The input vector must have {WIDTH} bits");
        debug_assert_eq!(
            round_constants.len(),
            NUM_ROUNDS,
            "The round constants vector must have {NUM_ROUNDS} elements"
        );

        // Partition the input into 64-bit chunks.
        let mut a = input.chunks(64).map(U64::from_bits_le).collect::<Vec<_>>();
        // Permute the input.
        for round_constant in round_constants.iter().take(NUM_ROUNDS) {
            a = Self::round(a, round_constant, rotl);
        }
        // Return the permuted input.
        a.into_iter().flat_map(|e| e.to_bits_le()).collect()
    }

    /// The round function `R` is defined as follows:
    /// ```text
    /// R = ι ◦ χ ◦ π ◦ ρ ◦ θ
    /// ```
    /// where `◦` denotes function composition.
    fn round(a: Vec<U64<E>>, round_constant: &U64<E>, rotl: &[usize]) -> Vec<U64<E>> {
        debug_assert_eq!(a.len(), MODULO * MODULO, "The input vector 'a' must have {} elements", MODULO * MODULO);

        const WEIGHT: usize = MODULO - 1;

        /* The first part of Algorithm 3, θ:
         *
         * for x = 0 to 4 do
         *   C[x] = a[x, 0]
         *   for y = 1 to 4 do
         *     C[x] = C[x] ⊕ a[x, y]
         *   end for
         * end for
         */
        let mut c = Vec::with_capacity(WEIGHT);
        for x in 0..MODULO {
            c.push(&a[x] ^ &a[x + MODULO] ^ &a[x + (2 * MODULO)] ^ &a[x + (3 * MODULO)] ^ &a[x + (4 * MODULO)]);
        }

        /* The second part of Algorithm 3, θ:
         *
         * for x = 0 to 4 do
         *   D[x] = C[x−1] ⊕ ROT(C[x+1],1)
         *   for y = 0 to 4 do
         *     A[x, y] = a[x, y] ⊕ D[x]
         *   end for
         * end for
         */
        let mut d = Vec::with_capacity(WEIGHT);
        for x in 0..MODULO {
            d.push(&c[(x + 4) % MODULO] ^ Self::rotate_left(&c[(x + 1) % MODULO], 63));
        }
        let mut a_1 = Vec::with_capacity(WEIGHT * WEIGHT);
        for y in 0..MODULO {
            for x in 0..MODULO {
                a_1.push(&a[x + (y * MODULO)] ^ &d[x]);
            }
        }

        /* Algorithm 4, π:
         *
         * for x = 0 to 4 do
         *   for y = 0 to 4 do
         *     (X, Y) = (y, (2*x + 3*y) mod 5)
         *     A[X, Y] = a[x, y]
         *   end for
         * end for
         *
         * Algorithm 5, ρ:
         *
         * A[0, 0] = a[0, 0]
         * (x, y) = (1, 0)
         * for t = 0 to 23 do
         *   A[x, y] = ROT(a[x, y], (t + 1)(t + 2)/2)
         *   (x, y) = (y, (2*x + 3*y) mod 5)
         * end for
         */
        let mut a_2 = a_1.clone();
        for y in 0..MODULO {
            for x in 0..MODULO {
                // This step combines the π and ρ steps into one.
                a_2[y + ((((2 * x) + (3 * y)) % MODULO) * MODULO)] =
                    Self::rotate_left(&a_1[x + (y * MODULO)], rotl[x + (y * MODULO)]);
            }
        }

        /* Algorithm 2, χ:
         *
         * for y = 0 to 4 do
         *   for x = 0 to 4 do
         *     A[x, y] = a[x, y] ⊕ ((¬a[x+1, y]) ∧ a[x+2, y])
         *   end for
         * end for
         */
        let mut a_3 = Vec::with_capacity(WEIGHT * WEIGHT);
        for y in 0..MODULO {
            for x in 0..MODULO {
                let a = &a_2[x + (y * MODULO)];
                let b = &a_2[((x + 1) % MODULO) + (y * MODULO)];
                let c = &a_2[((x + 2) % MODULO) + (y * MODULO)];
                a_3.push(a ^ ((!b) & c));
            }
        }

        /* ι:
         *
         * A[0, 0] = A[0, 0] ⊕ RC
         */
        a_3[0] = &a_3[0] ^ round_constant;
        a_3
    }

    /// Performs a rotate left operation on the given `u64` value.
    fn rotate_left(value: &U64<E>, n: usize) -> U64<E> {
        // Perform the rotation.
        let bits_le = value.to_bits_le();
        let bits_le = bits_le.iter().skip(n).chain(bits_le.iter()).take(64).cloned().collect::<Vec<_>>();
        // Return the rotated value.
        U64::from_bits_le(&bits_le)
    }
}

#[cfg(all(test, console))]
mod tests {
    use super::*;
    use console::{Console, Rng};
    use snarkvm_circuit_types::environment::Circuit;

    const ITERATIONS: usize = 3;

    macro_rules! check_equivalence {
        ($console:expr, $circuit:expr) => {
            use console::Hash as H;

            let rng = &mut TestRng::default();

            let mut input_sizes = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 16, 32, 64, 128, 256, 512, 1024];
            input_sizes.extend((0..5).map(|_| rng.gen_range(1..1024)));

            for num_inputs in input_sizes {
                println!("Checking equivalence for {num_inputs} inputs");

                // Prepare the preimage.
                let native_input = (0..num_inputs).map(|_| Uniform::rand(rng)).collect::<Vec<bool>>();
                let input = native_input.iter().map(|v| Boolean::<Circuit>::new(Mode::Private, *v)).collect::<Vec<_>>();

                // Compute the console hash.
                let expected = $console.hash(&native_input).expect("Failed to hash console input");

                // Compute the circuit hash.
                let candidate = $circuit.hash(&input);
                assert_eq!(expected, candidate.eject_value());
                Circuit::reset();
            }
        };
    }

    fn check_hash(
        mode: Mode,
        num_inputs: usize,
        num_constants: u64,
        num_public: u64,
        num_private: u64,
        num_constraints: u64,
        rng: &mut TestRng,
    ) {
        use console::Hash as H;

        let native = console::Keccak256::<Console>::new();
        let keccak = Keccak256::<Circuit>::new();

        for i in 0..ITERATIONS {
            // Prepare the preimage.
            let native_input = (0..num_inputs).map(|_| Uniform::rand(rng)).collect::<Vec<bool>>();
            let input = native_input.iter().map(|v| Boolean::<Circuit>::new(mode, *v)).collect::<Vec<_>>();

            // Compute the native hash.
            let expected = native.hash(&native_input).expect("Failed to hash native input");

            // Compute the circuit hash.
            Circuit::scope(format!("Keccak {mode} {i}"), || {
                let candidate = keccak.hash(&input);
                assert_eq!(expected, candidate.eject_value());
                let case = format!("(mode = {mode}, num_inputs = {num_inputs})");
                assert_scope!(case, num_constants, num_public, num_private, num_constraints);
            });
            Circuit::reset();
        }
    }

    #[test]
    fn test_keccak_256_hash_constant() {
        let mut rng = TestRng::default();

        check_hash(Mode::Constant, 1, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 2, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 3, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 4, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 5, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 6, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 7, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 8, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 16, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 32, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 64, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 128, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 256, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 511, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 512, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 513, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 1023, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 1024, 0, 0, 0, 0, &mut rng);
        check_hash(Mode::Constant, 1025, 0, 0, 0, 0, &mut rng);
    }

    #[test]
    fn test_keccak_256_hash_public() {
        let mut rng = TestRng::default();

        check_hash(Mode::Public, 1, 0, 0, 138157, 138157, &mut rng);
        check_hash(Mode::Public, 2, 0, 0, 139108, 139108, &mut rng);
        check_hash(Mode::Public, 3, 0, 0, 139741, 139741, &mut rng);
        check_hash(Mode::Public, 4, 0, 0, 140318, 140318, &mut rng);
        check_hash(Mode::Public, 5, 0, 0, 140879, 140879, &mut rng);
        check_hash(Mode::Public, 6, 0, 0, 141350, 141350, &mut rng);
        check_hash(Mode::Public, 7, 0, 0, 141787, 141787, &mut rng);
        check_hash(Mode::Public, 8, 0, 0, 142132, 142132, &mut rng);
        check_hash(Mode::Public, 16, 0, 0, 144173, 144173, &mut rng);
        check_hash(Mode::Public, 32, 0, 0, 145394, 145394, &mut rng);
        check_hash(Mode::Public, 64, 0, 0, 146650, 146650, &mut rng);
        check_hash(Mode::Public, 128, 0, 0, 149248, 149248, &mut rng);
        check_hash(Mode::Public, 256, 0, 0, 150848, 150848, &mut rng);
        check_hash(Mode::Public, 512, 0, 0, 151424, 151424, &mut rng);
        check_hash(Mode::Public, 1024, 0, 0, 152448, 152448, &mut rng);
    }

    #[test]
    fn test_keccak_256_hash_private() {
        let mut rng = TestRng::default();

        check_hash(Mode::Private, 1, 0, 0, 138157, 138157, &mut rng);
        check_hash(Mode::Private, 2, 0, 0, 139108, 139108, &mut rng);
        check_hash(Mode::Private, 3, 0, 0, 139741, 139741, &mut rng);
        check_hash(Mode::Private, 4, 0, 0, 140318, 140318, &mut rng);
        check_hash(Mode::Private, 5, 0, 0, 140879, 140879, &mut rng);
        check_hash(Mode::Private, 6, 0, 0, 141350, 141350, &mut rng);
        check_hash(Mode::Private, 7, 0, 0, 141787, 141787, &mut rng);
        check_hash(Mode::Private, 8, 0, 0, 142132, 142132, &mut rng);
        check_hash(Mode::Private, 16, 0, 0, 144173, 144173, &mut rng);
        check_hash(Mode::Private, 32, 0, 0, 145394, 145394, &mut rng);
        check_hash(Mode::Private, 64, 0, 0, 146650, 146650, &mut rng);
        check_hash(Mode::Private, 128, 0, 0, 149248, 149248, &mut rng);
        check_hash(Mode::Private, 256, 0, 0, 150848, 150848, &mut rng);
        check_hash(Mode::Private, 512, 0, 0, 151424, 151424, &mut rng);
        check_hash(Mode::Private, 1024, 0, 0, 152448, 152448, &mut rng);
    }

    #[test]
    fn test_keccak_224_equivalence() {
        check_equivalence!(console::Keccak224::<Console>::new(), Keccak224::<Circuit>::new());
    }

    #[test]
    fn test_keccak_256_equivalence() {
        check_equivalence!(console::Keccak256::<Console>::new(), Keccak256::<Circuit>::new());
    }

    #[test]
    fn test_keccak_384_equivalence() {
        check_equivalence!(console::Keccak384::<Console>::new(), Keccak384::<Circuit>::new());
    }

    #[test]
    fn test_keccak_512_equivalence() {
        check_equivalence!(console::Keccak512::<Console>::new(), Keccak512::<Circuit>::new());
    }

    #[test]
    fn test_sha3_224_equivalence() {
        check_equivalence!(console::Sha3_224::<Console>::new(), Sha3_224::<Circuit>::new());
    }

    #[test]
    fn test_sha3_256_equivalence() {
        check_equivalence!(console::Sha3_256::<Console>::new(), Sha3_256::<Circuit>::new());
    }

    #[test]
    fn test_sha3_384_equivalence() {
        check_equivalence!(console::Sha3_384::<Console>::new(), Sha3_384::<Circuit>::new());
    }

    #[test]
    fn test_sha3_512_equivalence() {
        check_equivalence!(console::Sha3_512::<Console>::new(), Sha3_512::<Circuit>::new());
    }
}
