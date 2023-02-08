use std::env;

use doublepir_rs::doublepir::DoublePirServer;
use doublepir_rs::pir::PirServer;

fn main() {
    let args: Vec<String> = env::args().collect();
    let num_entries: usize = args[1].parse().unwrap();
    let bits_per_entry: usize = args[2].parse().unwrap();
    let data_file_name: String = args[3].parse().unwrap();
    assert_eq!(bits_per_entry, 1);

    // let file = File::open(&data_file_name).expect("File did not exist");

    let mut server = DoublePirServer::new(num_entries, bits_per_entry);
    server.load_data_fast(&data_file_name);

    server.save_to_files(&data_file_name);
}
