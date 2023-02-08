use std::iter::Copied;

use crate::database::{Db, DbInfo};
use crate::matrix::{Matrix, SquishParams};

pub type State = Vec<Matrix>;

const MAX_LEN: u32 = 1 << 28;

pub trait Serialize {
    fn serialize(&self) -> Vec<u8>;
}

pub trait Deserialize<'a, I: Iterator<Item = u8>>
where
    Self: Sized,
{
    fn deserialize_iter(iter: &mut I) -> Self;
}

pub trait DeserializeSlice<'a>
where
    Self: Deserialize<'a, Copied<std::slice::Iter<'a, u8>>>,
{
    fn deserialize(slc: &'a [u8]) -> Self;
}

impl<'a, T> DeserializeSlice<'a> for T
where
    T: Deserialize<'a, Copied<std::slice::Iter<'a, u8>>>,
{
    fn deserialize(slc: &'a [u8]) -> Self {
        Self::deserialize_iter(&mut slc.iter().copied())
    }
}

fn write_u32(out: &mut Vec<u8>, val: u32) {
    out.extend_from_slice(&val.to_be_bytes());
}

// fn read_u32(inp: &[u8]) -> (&[u8], u32) {
//     let val = u32::from_be_bytes(inp[0..4].try_into().unwrap());
//     (&inp[4..], val)
// }

fn read_u32_iter<'a, I: Iterator<Item = u8>>(iter: &mut I) -> u32 {
    let bytes = [
        iter.next().unwrap(),
        iter.next().unwrap(),
        iter.next().unwrap(),
        iter.next().unwrap(),
    ];
    u32::from_be_bytes(bytes)
}

impl Serialize for Matrix {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_u32(&mut out, self.rows as u32);
        write_u32(&mut out, self.cols as u32);
        for v in self.data.iter() {
            write_u32(&mut out, *v);
        }
        out
    }
}

impl<'a, I> Deserialize<'a, I> for Matrix
where
    I: Iterator<Item = u8>,
{
    fn deserialize_iter(iter: &mut I) -> Self {
        let rows = read_u32_iter(iter);
        let cols = read_u32_iter(iter);
        assert!(rows < MAX_LEN);
        assert!(cols < MAX_LEN);
        let mut mat = Matrix::new(rows as usize, cols as usize);
        for i in 0..(rows * cols) as usize {
            mat.data[i] = read_u32_iter(iter);
        }
        mat
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        write_u32(&mut out, self.len() as u32);
        for val in self {
            out.extend(&val.serialize())
        }
        out
    }
}

impl<'a, I: Iterator<Item = u8>, T: Deserialize<'a, I>> Deserialize<'a, I> for Vec<T> {
    fn deserialize_iter(iter: &mut I) -> Self {
        let len = read_u32_iter(iter);
        assert!(len < MAX_LEN);
        let mut out = Vec::with_capacity(len as usize);
        for _ in 0..len {
            let val = T::deserialize_iter(iter);
            out.push(val);
        }
        out
    }
}

trait TakeN {
    fn take_n<const N: usize>(&mut self) -> [u8; N];
}

impl<'a, I> TakeN for I
where
    I: Iterator<Item = u8>,
{
    fn take_n<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        for i in 0..N {
            out[i] = self.next().unwrap();
        }
        out
    }
}

impl Serialize for DbInfo {
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(self.num_entries.to_be_bytes());
        out.extend(self.bits_per_entry.to_be_bytes());
        out.extend(self.packing.to_be_bytes());
        out.extend(self.ne.to_be_bytes());
        out.extend(self.x.to_be_bytes());
        out.extend(self.p.to_be_bytes());
        out.extend(self.logq.to_be_bytes());
        out.extend(self.squish_params.basis.to_be_bytes());
        out.extend(self.squish_params.delta.to_be_bytes());
        out.extend(self.orig_cols.to_be_bytes());
        out
    }
}

impl<'a, I: Iterator<Item = u8>> Deserialize<'a, I> for DbInfo {
    fn deserialize_iter(iter: &mut I) -> Self {
        let num_entries = usize::from_be_bytes(iter.take_n());
        let bits_per_entry = u64::from_be_bytes(iter.take_n());
        let packing = usize::from_be_bytes(iter.take_n());
        let ne = usize::from_be_bytes(iter.take_n());
        let x = usize::from_be_bytes(iter.take_n());
        let p = u64::from_be_bytes(iter.take_n());
        let logq = u64::from_be_bytes(iter.take_n());
        let basis = u64::from_be_bytes(iter.take_n());
        let delta = usize::from_be_bytes(iter.take_n());
        let orig_cols = usize::from_be_bytes(iter.take_n());
        let out = Self {
            num_entries,
            bits_per_entry,
            packing,
            ne,
            x,
            p,
            logq,
            squish_params: SquishParams { basis, delta },
            orig_cols,
        };
        out
    }
}

impl Serialize for Db {
    fn serialize(&self) -> Vec<u8> {
        let mut info = self.info.serialize();
        let data = self.data.serialize();
        info.extend(&data);
        return info;
    }
}

// impl<'a, I: Iterator<Item = u8>> Deserialize<'a, I> for Db {
//     fn deserialize_iter(iter: &mut I) -> Self {
//         let info = DbInfo::deserialize_iter(iter);
//         // let data = Matrix::new(1, 1);
//         let data = Matrix::deserialize_iter(iter);
//         Self {
//             info,
//             data,
//             raw_data: Vec::new(),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use crate::{
        matrix::Matrix,
        serializer::{serializer::State, DeserializeSlice, Serialize},
    };

    #[test]
    fn serialization_is_inverse_of_itself() {
        let s = vec![
            Matrix::random(10, 35),
            Matrix::random(7, 1),
            Matrix::random(1, 7),
        ];
        let s1 = s.serialize();
        let s2 = State::deserialize(&s1.clone());
        let s3 = s2.serialize();
        let s4 = State::deserialize(&s3.clone());

        assert_eq!(s, s2);
        assert_eq!(s1, s3);
        assert_eq!(s, s4);
    }

    // #[test]
    // fn db_serialization_works() {
    //     let num_entries = 1usize << 22;
    //     let bits_per_entry = 1;
    //     let params = pick_params(num_entries, bits_per_entry, SEC_PARAM, LOGQ);

    //     let mut random_db = Db::random(num_entries, bits_per_entry, &params);
    //     let serialized_db = random_db.serialize();
    //     let deserialized_db = Db::deserialize(&serialized_db);
    //     assert_eq!(random_db, deserialized_db);

    //     random_db.squish();
    //     let serialized_db = random_db.serialize();
    //     let deserialized_db = Db::deserialize(&serialized_db);
    //     assert_eq!(random_db, deserialized_db);
    // }
}
