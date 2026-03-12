pub trait RandomNumberGenerator {
    fn get_block(&mut self, output: &mut [u8]);
}

impl<T> RandomNumberGenerator for T
where
    T: crate::straw::Straw,
{
    fn get_block(&mut self, output: &mut [u8]) {
        let mut offset = 0usize;
        while offset < output.len() {
            let got = self.get(&mut output[offset..]);
            if got <= 0 {
                break;
            }
            offset += got as usize;
        }
    }
}
