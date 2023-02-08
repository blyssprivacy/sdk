pub trait PirServer {
    fn new(num_entries: usize, bits_per_entry: usize) -> Self;
    fn load_data<'a, I: Iterator<Item = u8>>(&mut self, data: I);
    fn get_hint(&self) -> Vec<u8>;
    fn answer(&self, query: &[u8]) -> Vec<u8>;
}

pub trait PirClient {
    fn new(num_entries: usize, bits_per_entry: usize) -> Self;
    fn load_hint(&mut self, hint: &[u8]);
    fn generate_query(&self, index: usize) -> (Vec<u8>, Vec<u8>);
    fn decode_response(&self, response: &[u8], index: usize, client_query_data: &[u8]) -> Vec<u8>;
}
