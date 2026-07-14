use merlin_protocol::crc32_mpeg2;

fn main() {
    assert_eq!(
        crc32_mpeg2::checksum(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39]),
        0x0376_E6E7
    );
    println!("MerlinProtocol OK");
}
