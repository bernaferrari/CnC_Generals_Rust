#[derive(Clone, Debug)]
pub struct RC4Class {
    state: [u8; 256],
    x: u8,
    y: u8,
}

impl RC4Class {
    pub fn new() -> Self {
        Self {
            state: [0u8; 256],
            x: 0,
            y: 0,
        }
    }

    pub fn prepare_key(&mut self, key_data: &[u8]) {
        match key_data.len() {
            8 => self.prepare_key_8bytes(key_data),
            16 => self.prepare_key_16bytes(key_data),
            0 => {
                self.state = [0u8; 256];
                self.x = 0;
                self.y = 0;
            }
            _ => {
                self.state = RC4_TABLE_INIT;
                self.x = 0;
                self.y = 0;
                let mut index1: u8 = 0;
                let mut index2: u8 = 0;
                for counter in 0..256u16 {
                    let idx = counter as usize;
                    index2 = index2
                        .wrapping_add(key_data[index1 as usize])
                        .wrapping_add(self.state[idx]);
                    self.state.swap(idx, index2 as usize);
                    index1 = index1.wrapping_add(1);
                    if index1 as usize >= key_data.len() {
                        index1 = 0;
                    }
                }
            }
        }
    }

    pub fn rc4(&mut self, buffer: &mut [u8]) {
        let mut x = self.x;
        let mut y = self.y;

        for byte in buffer.iter_mut() {
            x = x.wrapping_add(1);
            y = y.wrapping_add(self.state[x as usize]);
            self.state.swap(x as usize, y as usize);
            let idx = self.state[x as usize].wrapping_add(self.state[y as usize]);
            *byte ^= self.state[idx as usize];
        }

        self.x = x;
        self.y = y;
    }

    pub fn state(&self) -> (&[u8; 256], u8, u8) {
        (&self.state, self.x, self.y)
    }

    fn prepare_key_8bytes(&mut self, key_data: &[u8]) {
        self.state = RC4_TABLE_INIT;
        self.x = 0;
        self.y = 0;
        let mut index1: u8 = 0;
        let mut index2: u8 = 0;
        for counter in 0..256u16 {
            let idx = counter as usize;
            index2 = index2
                .wrapping_add(key_data[index1 as usize])
                .wrapping_add(self.state[idx]);
            self.state.swap(idx, index2 as usize);
            index1 = (index1.wrapping_add(1)) & 0x07;
        }
    }

    fn prepare_key_16bytes(&mut self, key_data: &[u8]) {
        self.state = RC4_TABLE_INIT;
        self.x = 0;
        self.y = 0;
        let mut index1: u8 = 0;
        let mut index2: u8 = 0;
        for counter in 0..256u16 {
            let idx = counter as usize;
            index2 = index2
                .wrapping_add(key_data[index1 as usize])
                .wrapping_add(self.state[idx]);
            self.state.swap(idx, index2 as usize);
            index1 = (index1.wrapping_add(1)) & 0x0F;
        }
    }
}

impl Default for RC4Class {
    fn default() -> Self {
        Self::new()
    }
}

pub type Rc4 = RC4Class;

const RC4_TABLE_INIT: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0u16;
    while i < 256 {
        table[i as usize] = i as u8;
        i += 1;
    }
    table
};
