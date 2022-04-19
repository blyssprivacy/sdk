use crate::{
    arith::*, discrete_gaussian::*, gadget::*, number_theory::*, params::*, poly::*, util::*,
};
use rand::Rng;
use std::iter::once;

fn serialize_polymatrix(vec: &mut Vec<u8>, a: &PolyMatrixRaw) {
    for i in 0..a.rows * a.cols * a.params.poly_len {
        vec.extend_from_slice(&u64::to_ne_bytes(a.data[i]));
    }
}

fn serialize_vec_polymatrix(vec: &mut Vec<u8>, a: &Vec<PolyMatrixRaw>) {
    for i in 0..a.len() {
        serialize_polymatrix(vec, &a[i]);
    }
}

pub struct PublicParameters<'a> {
    pub v_packing: Vec<PolyMatrixNTT<'a>>, // Ws
    pub v_expansion_left: Option<Vec<PolyMatrixNTT<'a>>>,
    pub v_expansion_right: Option<Vec<PolyMatrixNTT<'a>>>,
    pub v_conversion: Option<Vec<PolyMatrixNTT<'a>>>, // V
}

impl<'a> PublicParameters<'a> {
    pub fn init(params: &'a Params) -> Self {
        if params.expand_queries {
            PublicParameters {
                v_packing: Vec::new(),
                v_expansion_left: Some(Vec::new()),
                v_expansion_right: Some(Vec::new()),
                v_conversion: Some(Vec::new()),
            }
        } else {
            PublicParameters {
                v_packing: Vec::new(),
                v_expansion_left: None,
                v_expansion_right: None,
                v_conversion: None,
            }
        }
    }

    fn from_ntt_alloc_vec(v: &Vec<PolyMatrixNTT<'a>>) -> Option<Vec<PolyMatrixRaw<'a>>> {
        Some(v.iter().map(from_ntt_alloc).collect())
    }

    fn from_ntt_alloc_opt_vec(
        v: &Option<Vec<PolyMatrixNTT<'a>>>,
    ) -> Option<Vec<PolyMatrixRaw<'a>>> {
        Some(v.as_ref()?.iter().map(from_ntt_alloc).collect())
    }

    pub fn to_raw(&self) -> Vec<Option<Vec<PolyMatrixRaw>>> {
        vec![
            Self::from_ntt_alloc_vec(&self.v_packing),
            Self::from_ntt_alloc_opt_vec(&self.v_expansion_left),
            Self::from_ntt_alloc_opt_vec(&self.v_expansion_right),
            Self::from_ntt_alloc_opt_vec(&self.v_conversion),
        ]
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for v in self.to_raw().iter() {
            if v.is_some() {
                serialize_vec_polymatrix(&mut data, v.as_ref().unwrap());
            }
        }
        data
    }
}

pub struct Query<'a> {
    pub ct: Option<PolyMatrixRaw<'a>>,
    pub v_buf: Option<Vec<u64>>,
    pub v_ct: Option<Vec<PolyMatrixRaw<'a>>>,
}

impl<'a> Query<'a> {
    pub fn empty() -> Self {
        Query {
            ct: None,
            v_ct: None,
            v_buf: None,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut data = Vec::new();
        if self.ct.is_some() {
            let ct = self.ct.as_ref().unwrap();
            serialize_polymatrix(&mut data, &ct);
        }
        if self.v_buf.is_some() {
            let v_buf = self.v_buf.as_ref().unwrap();
            data.extend(v_buf.iter().map(|x| x.to_ne_bytes()).flatten());
        }
        if self.v_ct.is_some() {
            let v_ct = self.v_ct.as_ref().unwrap();
            for x in v_ct {
                serialize_polymatrix(&mut data, x);
            }
        }
        data
    }
}

pub struct Client<'a, TRng: Rng> {
    params: &'a Params,
    sk_gsw: PolyMatrixRaw<'a>,
    pub sk_reg: PolyMatrixRaw<'a>,
    sk_gsw_full: PolyMatrixRaw<'a>,
    sk_reg_full: PolyMatrixRaw<'a>,
    dg: DiscreteGaussian<'a, TRng>,
    pub g: usize,
    pub stop_round: usize,
}

fn matrix_with_identity<'a>(p: &PolyMatrixRaw<'a>) -> PolyMatrixRaw<'a> {
    assert_eq!(p.cols, 1);
    let mut r = PolyMatrixRaw::zero(p.params, p.rows, p.rows + 1);
    r.copy_into(p, 0, 0);
    r.copy_into(&PolyMatrixRaw::identity(p.params, p.rows, p.rows), 0, 1);
    r
}

fn params_with_moduli(params: &Params, moduli: &Vec<u64>) -> Params {
    Params::init(
        params.poly_len,
        moduli,
        params.noise_width,
        params.n,
        params.pt_modulus,
        params.q2_bits,
        params.t_conv,
        params.t_exp_left,
        params.t_exp_right,
        params.t_gsw,
        params.expand_queries,
        params.db_dim_1,
        params.db_dim_2,
        params.instances,
        params.db_item_size,
    )
}

impl<'a, TRng: Rng> Client<'a, TRng> {
    pub fn init(params: &'a Params, rng: &'a mut TRng) -> Self {
        let sk_gsw_dims = params.get_sk_gsw();
        let sk_reg_dims = params.get_sk_reg();
        let sk_gsw = PolyMatrixRaw::zero(params, sk_gsw_dims.0, sk_gsw_dims.1);
        let sk_reg = PolyMatrixRaw::zero(params, sk_reg_dims.0, sk_reg_dims.1);
        let sk_gsw_full = matrix_with_identity(&sk_gsw);
        let sk_reg_full = matrix_with_identity(&sk_reg);
        let dg = DiscreteGaussian::init(params, rng);

        let further_dims = params.db_dim_2;
        let num_expanded = 1usize << params.db_dim_1;
        let num_bits_to_gen = params.t_gsw * further_dims + num_expanded;
        let g = log2_ceil_usize(num_bits_to_gen);
        let stop_round = log2_ceil_usize(params.t_gsw * further_dims);
        Self {
            params,
            sk_gsw,
            sk_reg,
            sk_gsw_full,
            sk_reg_full,
            dg,
            g,
            stop_round,
        }
    }

    pub fn get_rng(&mut self) -> &mut TRng {
        &mut self.dg.rng
    }

    fn get_fresh_gsw_public_key(&mut self, m: usize) -> PolyMatrixRaw<'a> {
        let params = self.params;
        let n = params.n;

        let a = PolyMatrixRaw::random_rng(params, 1, m, self.get_rng());
        let e = PolyMatrixRaw::noise(params, n, m, &mut self.dg);
        let a_inv = -&a;
        let b_p = &self.sk_gsw.ntt() * &a.ntt();
        let b = &e.ntt() + &b_p;
        let p = stack(&a_inv, &b.raw());
        p
    }

    fn get_regev_sample(&mut self) -> PolyMatrixNTT<'a> {
        let params = self.params;
        let a = PolyMatrixRaw::random_rng(params, 1, 1, self.get_rng());
        let e = PolyMatrixRaw::noise(params, 1, 1, &mut self.dg);
        let b_p = &self.sk_reg.ntt() * &a.ntt();
        let b = &e.ntt() + &b_p;
        let mut p = PolyMatrixNTT::zero(params, 2, 1);
        p.copy_into(&(-&a).ntt(), 0, 0);
        p.copy_into(&b, 1, 0);
        p
    }

    fn get_fresh_reg_public_key(&mut self, m: usize) -> PolyMatrixNTT<'a> {
        let params = self.params;

        let mut p = PolyMatrixNTT::zero(params, 2, m);

        for i in 0..m {
            p.copy_into(&self.get_regev_sample(), 0, i);
        }
        p
    }

    fn encrypt_matrix_gsw(&mut self, ag: &PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        let mx = ag.cols;
        let p = self.get_fresh_gsw_public_key(mx);
        let res = &(p.ntt()) + &(ag.pad_top(1));
        res
    }

    pub fn encrypt_matrix_reg(&mut self, a: &PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        let m = a.cols;
        let p = self.get_fresh_reg_public_key(m);
        &p + &a.pad_top(1)
    }

    pub fn decrypt_matrix_reg(&mut self, a: &PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        &self.sk_reg_full.ntt() * a
    }

    pub fn decrypt_matrix_gsw(&mut self, a: &PolyMatrixNTT<'a>) -> PolyMatrixNTT<'a> {
        &self.sk_gsw_full.ntt() * a
    }

    fn generate_expansion_params(
        &mut self,
        num_exp: usize,
        m_exp: usize,
    ) -> Vec<PolyMatrixNTT<'a>> {
        let params = self.params;
        let g_exp = build_gadget(params, 1, m_exp);
        let g_exp_ntt = g_exp.ntt();
        let mut res = Vec::new();

        for i in 0..num_exp {
            let t = (params.poly_len / (1 << i)) + 1;
            let tau_sk_reg = automorph_alloc(&self.sk_reg, t);
            let prod = &tau_sk_reg.ntt() * &g_exp_ntt;
            let w_exp_i = self.encrypt_matrix_reg(&prod);
            res.push(w_exp_i);
        }
        res
    }

    pub fn generate_keys(&mut self) -> PublicParameters<'a> {
        let params = self.params;
        self.dg.sample_matrix(&mut self.sk_gsw);
        self.dg.sample_matrix(&mut self.sk_reg);
        self.sk_gsw_full = matrix_with_identity(&self.sk_gsw);
        self.sk_reg_full = matrix_with_identity(&self.sk_reg);
        let sk_reg_ntt = to_ntt_alloc(&self.sk_reg);
        let m_conv = params.m_conv();

        let mut pp = PublicParameters::init(params);

        // Params for packing
        let gadget_conv = build_gadget(params, 1, m_conv);
        let gadget_conv_ntt = to_ntt_alloc(&gadget_conv);
        for i in 0..params.n {
            let scaled = scalar_multiply_alloc(&sk_reg_ntt, &gadget_conv_ntt);
            let mut ag = PolyMatrixNTT::zero(params, params.n, m_conv);
            ag.copy_into(&scaled, i, 0);
            let w = self.encrypt_matrix_gsw(&ag);
            pp.v_packing.push(w);
        }

        if params.expand_queries {
            // Params for expansion

            pp.v_expansion_left = Some(self.generate_expansion_params(self.g, params.t_exp_left));
            pp.v_expansion_right =
                Some(self.generate_expansion_params(self.stop_round + 1, params.t_exp_right));

            // Params for converison
            let g_conv = build_gadget(params, 2, 2 * m_conv);
            let sk_reg_ntt = self.sk_reg.ntt();
            let sk_reg_squared_ntt = &sk_reg_ntt * &sk_reg_ntt;
            pp.v_conversion = Some(Vec::from_iter(once(PolyMatrixNTT::zero(
                params,
                2,
                2 * m_conv,
            ))));
            for i in 0..2 * m_conv {
                let sigma;
                if i % 2 == 0 {
                    let val = g_conv.get_poly(0, i)[0];
                    sigma = &sk_reg_squared_ntt * &single_poly(params, val).ntt();
                } else {
                    let val = g_conv.get_poly(1, i)[0];
                    sigma = &sk_reg_ntt * &single_poly(params, val).ntt();
                }
                let ct = self.encrypt_matrix_reg(&sigma);
                pp.v_conversion.as_mut().unwrap()[0].copy_into(&ct, 0, i);
            }
        }

        pp
    }

    pub fn generate_query(&mut self, idx_target: usize) -> Query<'a> {
        let params = self.params;
        let further_dims = params.db_dim_2;
        let idx_dim0 = idx_target / (1 << further_dims);
        let idx_further = idx_target % (1 << further_dims);
        let scale_k = params.modulus / params.pt_modulus;
        let bits_per = get_bits_per(params, params.t_gsw);

        let mut query = Query::empty();
        if params.expand_queries {
            // pack query into single ciphertext
            let mut sigma = PolyMatrixRaw::zero(params, 1, 1);
            sigma.data[2 * idx_dim0] = scale_k;
            for i in 0..further_dims as u64 {
                let bit: u64 = ((idx_further as u64) & (1 << i)) >> i;
                for j in 0..params.t_gsw {
                    let val = (1u64 << (bits_per * j)) * bit;
                    let idx = (i as usize) * params.t_gsw + (j as usize);
                    sigma.data[2 * idx + 1] = val;
                }
            }
            let inv_2_g_first = invert_uint_mod(1 << self.g, params.modulus).unwrap();
            let inv_2_g_rest = invert_uint_mod(1 << (self.stop_round + 1), params.modulus).unwrap();

            for i in 0..params.poly_len / 2 {
                sigma.data[2 * i] =
                    multiply_uint_mod(sigma.data[2 * i], inv_2_g_first, params.modulus);
                sigma.data[2 * i + 1] =
                    multiply_uint_mod(sigma.data[2 * i + 1], inv_2_g_rest, params.modulus);
            }

            query.ct = Some(from_ntt_alloc(
                &self.encrypt_matrix_reg(&to_ntt_alloc(&sigma)),
            ));
        } else {
            let num_expanded = 1 << params.db_dim_1;
            let mut sigma_v = Vec::<PolyMatrixNTT>::new();

            // generate regev ciphertexts
            let reg_cts_buf_words = num_expanded * 2 * params.poly_len;
            let mut reg_cts_buf = vec![0u64; reg_cts_buf_words];
            let mut reg_cts = Vec::<PolyMatrixNTT>::new();
            for i in 0..num_expanded {
                let value = ((i == idx_dim0) as u64) * scale_k;
                let sigma = PolyMatrixRaw::single_value(&params, value);
                reg_cts.push(self.encrypt_matrix_reg(&to_ntt_alloc(&sigma)));
            }
            // reorient into server's preferred indexing
            reorient_reg_ciphertexts(self.params, reg_cts_buf.as_mut_slice(), &reg_cts);

            // generate GSW ciphertexts
            for i in 0..further_dims {
                let bit = ((idx_further as u64) & (1 << (i as u64))) >> (i as u64);
                let mut ct_gsw = PolyMatrixNTT::zero(&params, 2, 2 * params.t_gsw);

                for j in 0..params.t_gsw {
                    let value = (1u64 << (bits_per * j)) * bit;
                    let sigma = PolyMatrixRaw::single_value(&params, value);
                    let sigma_ntt = to_ntt_alloc(&sigma);
                    let ct = &self.encrypt_matrix_reg(&sigma_ntt);
                    ct_gsw.copy_into(ct, 0, 2 * j + 1);
                    let prod = &to_ntt_alloc(&self.sk_reg) * &sigma_ntt;
                    let ct = &self.encrypt_matrix_reg(&prod);
                    ct_gsw.copy_into(ct, 0, 2 * j);
                }
                sigma_v.push(ct_gsw);
            }

            query.v_buf = Some(reg_cts_buf);
            query.v_ct = Some(sigma_v.iter().map(|x| from_ntt_alloc(x)).collect());
        }
        query
    }

    pub fn decode_response(&self, data: &[u8]) -> Vec<u8> {
        /*
            0. NTT over q2 the secret key

            1. read first row in q2_bit chunks
            2. read rest in q1_bit chunks
            3. NTT over q2 the first row
            4. Multiply the results of (0) and (3)
            5. Divide and round correctly
        */
        let params = self.params;
        let p = params.pt_modulus;
        let p_bits = log2_ceil(params.pt_modulus);
        let q1 = 4 * params.pt_modulus;
        let q1_bits = log2_ceil(q1) as usize;
        let q2 = Q2_VALUES[params.q2_bits as usize];
        let q2_bits = params.q2_bits as usize;

        let q2_params = params_with_moduli(params, &vec![q2]);

        // this only needs to be done during keygen
        let mut sk_gsw_q2 = PolyMatrixRaw::zero(&q2_params, params.n, 1);
        for i in 0..params.poly_len * params.n {
            sk_gsw_q2.data[i] = recenter(self.sk_gsw.data[i], params.modulus, q2);
        }
        let mut sk_gsw_q2_ntt = PolyMatrixNTT::zero(&q2_params, params.n, 1);
        to_ntt(&mut sk_gsw_q2_ntt, &sk_gsw_q2);

        let mut result = PolyMatrixRaw::zero(&params, params.instances * params.n, params.n);

        let mut bit_offs = 0;
        for instance in 0..params.instances {
            // this must be done during decoding
            let mut first_row = PolyMatrixRaw::zero(&q2_params, 1, params.n);
            let mut rest_rows = PolyMatrixRaw::zero(&params, params.n, params.n);
            for i in 0..params.n * params.poly_len {
                first_row.data[i] = read_arbitrary_bits(data, bit_offs, q2_bits);
                bit_offs += q2_bits;
            }
            for i in 0..params.n * params.n * params.poly_len {
                rest_rows.data[i] = read_arbitrary_bits(data, bit_offs, q1_bits);
                bit_offs += q1_bits;
            }

            let mut first_row_q2 = PolyMatrixNTT::zero(&q2_params, 1, params.n);
            to_ntt(&mut first_row_q2, &first_row);

            let sk_prod = (&sk_gsw_q2_ntt * &first_row_q2).raw();

            let q1_i64 = q1 as i64;
            let q2_i64 = q2 as i64;
            let p_i128 = p as i128;
            for i in 0..params.n * params.n * params.poly_len {
                let mut val_first = sk_prod.data[i] as i64;
                if val_first >= q2_i64 / 2 {
                    val_first -= q2_i64;
                }
                let mut val_rest = rest_rows.data[i] as i64;
                if val_rest >= q1_i64 / 2 {
                    val_rest -= q1_i64;
                }

                let denom = (q2 * (q1 / p)) as i64;

                let mut r = val_first * q1_i64;
                r += val_rest * q2_i64;

                // divide r by q2, rounding
                let sign: i64 = if r >= 0 { 1 } else { -1 };
                let mut res = ((r + sign * (denom / 2)) as i128) / (denom as i128);
                res = (res + (denom as i128 / p_i128) * (p_i128) + 2 * (p_i128)) % (p_i128);
                let idx = instance * params.n * params.n * params.poly_len + i;
                result.data[idx] = res as u64;
            }
        }

        // println!("{:?}", result.data);
        let trials = params.n * params.n;
        let chunks = params.instances * trials;
        let bytes_per_chunk = f64::ceil(params.db_item_size as f64 / chunks as f64) as usize;
        let logp = log2(params.pt_modulus);
        let modp_words_per_chunk = f64::ceil((bytes_per_chunk * 8) as f64 / logp as f64) as usize;
        println!("modp_words_per_chunk {:?}", modp_words_per_chunk);
        result.to_vec(p_bits as usize, modp_words_per_chunk)
    }
}

#[cfg(test)]
mod test {
    use rand::thread_rng;

    use super::*;

    fn assert_first8(m: &[u64], gold: [u64; 8]) {
        let got: [u64; 8] = m[0..8].try_into().unwrap();
        assert_eq!(got, gold);
    }

    fn get_params() -> Params {
        get_short_keygen_params()
    }

    #[test]
    fn init_is_correct() {
        let params = get_params();
        let mut rng = thread_rng();
        let client = Client::init(&params, &mut rng);

        assert_eq!(client.stop_round, 5);
        assert_eq!(client.g, 10);
        assert_eq!(*client.params, params);
    }

    #[test]
    fn keygen_is_correct() {
        let params = get_params();
        let mut seeded_rng = get_static_seeded_rng();
        let mut client = Client::init(&params, &mut seeded_rng);

        let public_params = client.generate_keys();

        assert_first8(
            public_params.v_conversion.unwrap()[0].data.as_slice(),
            [
                253586619, 247235120, 141892996, 163163429, 15531298, 200914775, 125109567,
                75889562,
            ],
        );

        assert_first8(
            client.sk_gsw.data.as_slice(),
            [1, 5, 0, 3, 1, 3, 66974689739603967, 3],
        );
    }
}
