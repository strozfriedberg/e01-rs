pub mod e01_reader;
mod generated;

#[allow(unused)]
mod test {
    use crate::e01_reader::E01Reader;
    use sha1::Digest;
    use sha1::Sha1;
    // use sha2::Digest;
    // use sha2::Sha256;
    use std::process::Command;

    fn do_hash(vmdk_path: &str) -> String /*hash*/ {
        let e01_reader = E01Reader::open(&vmdk_path).unwrap();

        //e01_reader.check_size();

        let mut hasher = Sha1::new();
        let mut buf: Vec<u8> = vec![0; 1048576];
        let mut offset = 0;
        // let mut sum = 0;
        // let mut sum_bytes = 0;
        while offset < e01_reader.total_size() {
            let buf_size = buf.len();
            let readed = match e01_reader.read_at_offset(offset, &mut buf[..buf_size]) {
                Ok(v) => v,
                Err(e) => {
                    panic!("{:?}", e);
                }
            };

            if readed == 0 {
                break;
            }

            // let mut hasher2 = Sha256::new();
            // hasher2.update(&buf[..readed]);
            // println!("{} {:X}", offset, hasher2.finalize());

            hasher.update(&buf[..readed]);

            // sum_bytes += buf[..readed].len();
            // sum += buf[..readed].iter().fold(0u32, |acc, x| acc + *x as u32);
            // if sum_bytes == 32768 {
            //     println!("{} sum of {} = {}", offset, sum_bytes, sum);
            //     sum_bytes = 0;
            //     sum = 0;
            // }

            offset += readed;
        }
        let result = hasher.finalize();
        format!("{:X}", result)
        //"".to_string()
    }

    #[test]
    fn test_all_images() {
        assert_eq!(
            do_hash("data/image.E01"),
            "E5C6C296485B1146FEAD7AD552E1C3CCFC00BFAB"
        );

        assert_eq!(
            do_hash("C:/temp/E01/mimage.E01"),
            "F8677BD8A38A12476AE655A9F9F5336C287603F7"
        );

        assert_eq!(
            do_hash("C:/temp/E01/multi.E01"),
            "F8677BD8A38A12476AE655A9F9F5336C287603F7"
        );
    }

    use ::ewf::*;
    fn do_hash_ewf(path: &str) -> String {
        let mut ewf = ewf::EWF::new(path).unwrap();
        let buffer_size: usize = 1048576;
        let mut hasher = Sha1::new();
        let mut data = ewf.read(buffer_size);
        let mut readed = 0;
        while data.len() > 0 {
            readed += data.len();
            hasher.update(&data);
            if data.len() < buffer_size {
                break;
            }
            data = ewf.read(buffer_size);
        }
        format!("{:X}", hasher.finalize())
    }

    // #[test]
    // fn test_2() {
    //     do_hash_by_buf("C:/temp/E01/test.E01");
    // }

    // fn do_hash_by_buf(path: &str) {
    //     let e01_reader = E01Reader::open(&path).unwrap();
    //     let mut ewf = ewf::EWF::new(path).unwrap();

    //     let buffer_size: usize = 1048576;

    //     e01_reader.check_size();

    //     let mut data;
    //     let mut data2: Vec<u8> = vec![0; buffer_size];
    //     let mut offset = 0;
    //     while offset < e01_reader.total_size() {
    //         data = ewf.read(data2.len());

    //         let readed = match e01_reader.read_at_offset(offset, &mut data2) {
    //             Ok(v) => v,
    //             Err(e) => {
    //                 panic!("{:?}", e);
    //             }
    //         };

    //         assert_eq!(data, data2[..readed]);

    //         if readed == 0 {
    //             break;
    //         }

    //         if data.len() < buffer_size {
    //             break;
    //         }

    //         offset += readed;
    //     }
    // }
}
