// bobbin_bits library used to eliminate redundant masking/range checking on function parameters.
use bobbin_bits::*;
use rand::Rng;

/// The CHIP-8 interpreter itself. Encapsulates memory, registers, the screen, and keyboard.
pub struct CPU{
    memory: [u8; 4096], // 4KB of addressable memory
    registers: [u8; 16], // 16 general-purpose registers, V0 through VF
    i: u16, // Special register
    dt: u8, // 8-bit delay timer
    pub st: u8, // 8-bit sound timer
    pc: u16, // 16-bit program counter
    sp: u8, // 8-bit stack pointer
    stack: [u16; 16], // 16 element stack
    pub screen: [[bool; 64]; 32], // 64x32 display, represented as 2D array of booleans
                                  // True indicates pixel should be lit, false indicates otherwise
    keyboard: [bool; 16], // 16 character keyboard, labelled 0 through F
                          // True indicates the character is being pressed, false indicates otherwise
}

/// Default font for CHIP-8 games, loaded into memory at address 0x0.
/// This consists of sixteen 8x5 sprites.
const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80  // F
];

impl CPU{
    /// Instantiates a CHIP-8 compatable CPU, with font data copied into memory.
    pub fn new() -> Self{
        let mut new_cpu: CPU = Self{
            memory: [0; 4096],
            registers: [0; 16],
            i: 0,
            dt: 0,
            st: 0,
            pc: 0x200,
            sp: 0,
            stack: [0; 16],
            screen: [[false; 64]; 32],
            keyboard: [false; 16],
        };

        new_cpu.memory[..0x50].copy_from_slice(&FONT);

        new_cpu
    }

    /// Pushes a new value onto the stack.
    fn push(&mut self, val: u16){
        self.stack[self.sp as usize] = val;
        self.sp += 1;
    }

    /// Pops the value from the top of the stack and returns it.
    fn pop(&mut self) -> u16 {
        self.sp -= 1;
        self.stack[self.sp as usize]
    }

    /// Joins three 4-byte numbers into one 12-byte number.
    /// For example, if a = `0xF`, b = `0xA`, c = `0x7`, the result is `0xFA7`.
    fn concat_digits(a: U4, b: U4, c: U4) -> U12 {
        let hex_string = format!("{:x}{:x}{:x}", a, b, c);
        let hex_number = u16::from_str_radix(&hex_string, 16).expect("Invalid string");
        
        hex_number.into()
    }

    /// Loads supplied ROM data into memory, starting at offset 0x200.
    pub fn load(&mut self, rom: &[u8]) {
        let end = 0x200 as usize + rom.len();
        self.memory[0x200 as usize..end].copy_from_slice(&rom);
    }

    /// Update the status of a given key. Must be called every frame by the graphics layer.
    /// If `state` is true, the key is pressed. Else, it is not.
    pub fn update_key(&mut self, key: U4, state: bool) {
        self.keyboard[key as usize] = state;
    }

    /// Performs one fetch-decode-execute cycle.
    pub fn step(&mut self) {
        // Fetch bytes (PC, PC + 1)
        let byte1 = self.memory[self.pc as usize];
        let byte2 = self.memory[(self.pc + 1) as usize];

        // Parse into four hex digits
        let digit1: U4 = (byte1 >> 4).into();
        let digit2: U4 = (byte1 & 0xF).into();
        let digit3: U4 = (byte2 >> 4).into();
        let digit4: U4 = (byte2 & 0xF).into();

        // Increment PC
        self.pc += 2;

        // Decode and execute instruction
        match(digit1, digit2, digit3, digit4){
            (U4::B0000, U4::B0000, U4::B1110, U4::B0000) => self.clear(), // 00E0
            (U4::B0000, U4::B0000, U4::B1110, U4::B1110) => self.ret(), // 00EE
            (U4::B0001, _, _, _) => self.jump(Self::concat_digits(digit2, digit3, digit4)), // 1nnn
            (U4::B0010, _, _, _) => self.call(Self::concat_digits(digit2, digit3, digit4)), // 2nnn
            (U4::B0011, _, _, _) => self.skip_if_equal(digit2, byte2), // 3xkk
            (U4::B0100, _, _, _) => self.skip_if_not_equal(digit2, byte2), // 4xkk
            (U4::B0101, _, _, U4::B0000) => self.skip_if_registers_equal(digit2, digit3), // 5xy0
            (U4::B0110, _, _, _) => self.copy_into_register(digit2, byte2), // 6xkk
            (U4::B0111, _, _, _) => self.increment_register(digit2, byte2), // 7xkk
            (U4::B1000, _, _, U4::B0000) => self.copy_register(digit2, digit3), // 8xy0
            (U4::B1000, _, _, U4::B0001) => self.or_registers(digit2, digit3), // 8xy1
            (U4::B1000, _, _, U4::B0010) => self.and_registers(digit2, digit3), // 8xy2
            (U4::B1000, _, _, U4::B0011) => self.xor_registers(digit2, digit3), // 8xy3
            (U4::B1000, _, _, U4::B0100) => self.add_registers(digit2, digit3), // 8xy4
            (U4::B1000, _, _, U4::B0101) => self.subtract_registers(digit2, digit3), // 8xy5
            (U4::B1000, _, _, U4::B0110) => self.right_shift_register(digit2), // 8xy6
            (U4::B1000, _, _, U4::B0111) => self.subtract_numeric_registers(digit2, digit3), // 8xy7
            (U4::B1000, _, _, U4::B1110) => self.left_shift_register(digit2), // 8xyE
            (U4::B1001, _, _, U4::B0000) => self.skip_if_registers_not_equal(digit2, digit3), // 9xy0
            (U4::B1010, _, _, _) => self.copy_into_i_register(Self::concat_digits(digit2, digit3, digit4)), // Annn
            (U4::B1011, _, _, _) => self.offset_register_jump(Self::concat_digits(digit2, digit3, digit4)), // Bnnn
            (U4::B1100, _, _, _) => self.generate_random_value(digit2, byte2), // Cxkk
            (U4::B1101, _, _, _) => self.draw(digit2, digit3, digit4), // Dxyn
            (U4::B1110, _, U4::B1001, U4::B1110) => self.skip_if_key_pressed(digit2), // Ex9E
            (U4::B1110, _, U4::B1010, U4::B0001) => self.skip_if_key_not_pressed(digit2), // ExA1
            (U4::B1111, _, U4::B0000, U4::B0111) => self.copy_dt_into_register(digit2), // Fx07
            (U4::B1111, _, U4::B0000, U4::B1010) => self.wait_for_key_press(digit2), // Fx0A
            (U4::B1111, _, U4::B0001, U4::B0101) => self.set_delay_timer(digit2), // Fx15
            (U4::B1111, _, U4::B0001, U4::B1000) => self.set_sound_timer(digit2), // Fx18
            (U4::B1111, _, U4::B0001, U4::B1110) => self.add_to_i_register(digit2), // Fx1E
            (U4::B1111, _, U4::B0010, U4::B1001) => self.get_digit_sprite_location(digit2), // Fx29
            (U4::B1111, _, U4::B0011, U4::B0011) => self.bcd_representation(digit2), // Fx33
            (U4::B1111, _, U4::B0101, U4::B0101) => self.copy_registers_to_memory(digit2), // Fx55
            (U4::B1111, _, U4::B0110, U4::B0101) => self.copy_memory_into_registers(digit2), // Fx65
            (U4::B1111, U4::B1111, U4::B1111, U4::B1111) => println!("Reached end"), // FFFF (temporary debug instruction)
            _ => println!("Error: illegal instruction {}{}", byte1, byte2),
        };
    }

    /// Tick the sound timer and delay timer, decreasing them by 1.
    /// This function must be called every 16.67ms (60Hz) by the graphics layer.
    pub fn tick(&mut self) {
        if self.st > 0 { self.st -= 1; }
        if self.dt > 0 { self.dt -= 1; }
    }

    // Documentation based on http://devernay.free.fr/hacks/chip8/C8TECH10.HTM

    /// Clears the display (opcode `00E0`).
    fn clear(&mut self){
        self.screen = [[false; 64]; 32];
    }

    /// Return from a subroutine (opcode `00EE`). 
    /// The program counter is set to the value at the top of the stack, and the stack pointer is decremented.
    fn ret(&mut self) {
        self.pc = self.pop();
    }
    
    /// Jump to `addr` (opcode `2nnn`).
    fn jump(&mut self, addr: U12) {
        self.pc = addr.into();
    }

    /// Calls a subroutine starting at `addr` (opcode `3nnn`).
    fn call(&mut self, addr: U12) {
        self.push(self.pc);
        self.pc = addr.into();
    }

    /// Skips the next instruction if Vx = kk (opcode `3xkk`), by incrementing the program counter by 2.
    fn skip_if_equal(&mut self, x: U4, kk: u8) {
        if self.registers[x as usize] == kk { self.pc += 2; }
    }

    /// Skips the next instruction if Vx != kk (opcode `4xkk`), by incrementing the program counter by 2.
    fn skip_if_not_equal(&mut self, x: U4, kk: u8) {
        if self.registers[x as usize] != kk { self.pc += 2; }
    }

    /// Skips the next instruction if Vx = Vy (opcode `5xy0`), by incrementing the program counter by 2.
    fn skip_if_registers_equal(&mut self, x: U4, y: U4) {
        if self.registers[x as usize] == self.registers[y as usize] { self.pc += 2; }
    }

    /// Sets Vx = kk (opcode `6xkk`).
    fn copy_into_register(&mut self, x: U4, kk: u8){
        self.registers[x as usize] = kk;
    }

    /// Sets Vx = Vx + kk (opcode `7xkk`).
    fn increment_register(&mut self, x: U4, kk: u8){
        let (result, _overflow) = self.registers[x as usize].overflowing_add(kk);
        self.registers[x as usize] = result;
    }

    /// Sets Vx = Vy (opcode `8xy0`).
    fn copy_register(&mut self, x: U4, y: U4){
        self.registers[x as usize] = self.registers[y as usize];
    }

    /// Sets Vx = Vx | Vy (opcode `8xy1`).
    fn or_registers(&mut self, x: U4, y: U4){
        self.registers[x as usize] = self.registers[x as usize] | self.registers[y as usize];
    }

    /// Sets Vx = Vx & Vy (opcode `8xy2`).
    fn and_registers(&mut self, x: U4, y: U4){
        self.registers[x as usize] = self.registers[x as usize] & self.registers[y as usize];
    }

    /// Sets Vx = Vx ^ Vy (opcode `8xy3`).
    fn xor_registers(&mut self, x: U4, y: U4){
        self.registers[x as usize] = self.registers[x as usize] ^ self.registers[y as usize];
    }

    /// Sets Vx = Vx + Vy (opcode `8xy4`).
    /// If the result is greater than 8 bits (i.e., > 255,) VF is set to 1, otherwise 0. 
    /// Only the lowest 8 bits of the result are kept, and stored in Vx.
    fn add_registers(&mut self, x: U4, y: U4){
        let (result, greater) = self.registers[x as usize].overflowing_add(self.registers[y as usize]);

        self.registers[x as usize] = result;
        // VF always updated after register written to, in event Vx = VF.
        self.registers[0xF] = greater.into();
    }

    /// Sets Vx = Vx - Vy (opcode `8xy5`) and VF = NOT borrow.
    /// If Vx > Vy, then VF is set to 1, otherwise 0. Subtraction is wrapped to avoid integer overflow.
    fn subtract_registers(&mut self, x: U4, y: U4){
        let (result, borrow) = self.registers[x as usize].overflowing_sub(self.registers[y as usize]);

        self.registers[x as usize] = result;
        // VF always updated after register written to, in event Vx = VF.
        self.registers[0xF] = !borrow as u8;
    }

    /// Sets Vx = Vx SHR 1 (opcode `8xy6`), in effect dividing by 2.
    /// VF is set equal to the bit that was shifted out.
    fn right_shift_register(&mut self, x: U4){
        let shifted_bit = self.registers[x as usize] & 1;
        self.registers[x as usize] = self.registers[x as usize] >> 1;
        self.registers[0xF] = shifted_bit;
    }

    /// Sets Vx = Vy - Vx (opcode `8xy7`) and VF = NOT borrow.
    /// If Vy > Vx, then VF is set to 1, otherwise 0. Subtraction is wrapped to avoid integer overflow.
    fn subtract_numeric_registers(&mut self, x: U4, y: U4){
        let (result, borrow) = self.registers[y as usize].overflowing_sub(self.registers[x as usize]);

        
        self.registers[x as usize] = result;
        // VF always updated after register written to, in event Vx = VF.
        self.registers[0xF] = !borrow as u8;
    }

    /// Sets Vx = Vx SHL 1 (opcode `8xyE`), in effect multiplying by 2.
    /// VF is set equal to the bit that was shifted out.
    fn left_shift_register(&mut self, x: U4){
        let msb = self.registers[x as usize] & 0x80;
        self.registers[x as usize] = self.registers[x as usize] << 1;
        self.registers[0xF] = if msb == 0x80 { 1 } else { 0 };
    }

    /// Skips the next instruction if Vx != Vy (opcode `9xy0`), by incrementing the program counter by 2.
    fn skip_if_registers_not_equal(&mut self, x: U4, y: U4) {
        if self.registers[x as usize] != self.registers[y as usize] { self.pc += 2; }
    }

    /// Sets I = nnn (opcode `Annn`).
    fn copy_into_i_register(&mut self, nnn: U12){
        self.i = nnn.into();
    }

    /// Jump to location nnn + V0 (opcode `Bnnn`), by changing the program counter.
    fn offset_register_jump(&mut self, nnn: U12){
        self.pc = u16::from(nnn) + self.registers[0] as u16;
    }

    /// Set Vx = rand & kk (opcode `Cxkk`), where rand is randomly generated (between 0 and 255).
    fn generate_random_value(&mut self, x: U4, kk: u8){
        let mut rng = rand::thread_rng();
        let random_value : u8 = rng.gen();
        self.registers[x as usize] = kk & random_value;
    }

    /// Display n-byte sprite starting at memory location I at (Vx, Vy), set VF = collision (opcode `Dxyn`).
    fn draw(&mut self, x: U4, y: U4, n: U4){
        self.registers[0xF] = 0;
        
        let starting_x = self.registers[x as usize];
        let starting_y = self.registers[y as usize];

        // Each byte is one row of the sprite
        for line in 0..(n as u8){
            // Get byte, iterate over bits from left-to-right
            let next_byte : u8 = self.memory[self.i as usize + line as usize];
            for x_iter in 0..8{
                // Get next bit via bit shift and mask, convert to bool
                // e.g., second column of sprite via 1 left shift and mask with 10000000
                let pixel: bool = ((next_byte << x_iter) & 0x80) != 0;

                // Wrap-around if past edge of screen
                let x_pos: usize = ((starting_x + x_iter) % 64) as usize;
                let y_pos: usize = ((starting_y + line) % 32) as usize;

                // Check for collision
                if pixel && self.screen[y_pos][x_pos] { self.registers[0xF] = 1; }

                // Update screen
                self.screen[y_pos][x_pos] = self.screen[y_pos][x_pos] ^ pixel;
            }
        }
    }

    /// Skips the next instruction if the key with the value of Vx is pressed (opcode `Ex9E`), by increasing the program counter by 2.
    fn skip_if_key_pressed(&mut self, x: U4){
        if self.keyboard[self.registers[x as usize] as usize] { self.pc += 2; }
    }

    /// Skips the next instruction if the key with the value of Vx is pressed (opcode `Ex9E`), by increasing the program counter by 2.
    fn skip_if_key_not_pressed(&mut self, x: U4){
        if !self.keyboard[self.registers[x as usize] as usize] { self.pc += 2; }
    }

    /// Set Vx = delay timer value (opcode Fx07).
    fn copy_dt_into_register(&mut self, x: U4){
        self.registers[x as usize] = self.dt;
    }

    /// Waits for a key press, then store the value of the key in Vx (opcode `Fx0A`). All execution stops until a key is pressed.
    /// Keys are polled in numerical order (0 through F). This is thread-safe; if no key is pressed, the instruction is repeated.
    fn wait_for_key_press(&mut self, x: U4){
        let mut pressed: bool = false;
        for key in 0..0xF{
            if self.keyboard[key as usize] { 
                self.registers[x as usize] = key;
                pressed = true; 
                break; 
            } 
        }

        // We can't hold an infinite loop here, else the graphics thread (e.g., SDL2) will hang
        //  and most OSes will think the interpreter is not responding. Instead, we'll reduce
        //  the program counter and "repeat" the instruction, in effect causing the program
        //  to not move forward to the next instruction until a key is pressed.
        if !pressed { self.pc -= 2; }
    }

    /// Set delay timer value = Vx (opcode Fx15).
    fn set_delay_timer(&mut self, x: U4){
        self.dt = self.registers[x as usize];
    }

    /// Set sound timer value = Vx (opcode Fx18).
    fn set_sound_timer(&mut self, x: U4){
        self.st = self.registers[x as usize];
    }

    /// Set I = I + Vx (opcode Fx1E).
    fn add_to_i_register(&mut self, x: U4){
        self.i = self.i + self.registers[x as usize] as u16;
    }

    /// Gets the address of the hexadecimal sprite corresponding to the value of Vx, and copies this into I (opcode `Fx29`).
    /// In this interpreter, said sprites are located starting at the beginning of memory (offset 0x0).
    fn get_digit_sprite_location(&mut self, x: U4){
        let target = self.registers[x as usize];

        // Each character sprite is 5 bytes starting at 0x0, so offset is easily calculated
        self.i = 5 * target as u16;
    }

    /// Stores the BCD representation of Vx in memory locations I, I+1, and I+2 (opcode `Fx33`).
    /// The interpreter takes the decimal value of Vx, and places the hundreds digit in memory at location in I, 
    ///   the tens digit at location I+1, and the ones digit at location I+2.
    fn bcd_representation(&mut self, x: U4){
        let value: f32 = self.registers[x as usize] as f32;

        // Get 100s, 10s, and 1s digits (in decimal)
        let hundreds = (value / 100.0).floor() as u8; // Get 100s digit by dividing by 100 and dropping decimal
        let tens = ((value / 10.0) % 10.0).floor() as u8; // Get 10s digit by dividing by 10, then retrieving 1s digit of result
        let ones = (value % 10.0) as u8; // Get 1s digit via modular arithmetic in Z10

        self.memory[self.i as usize] = hundreds;
        self.memory[self.i as usize + 1] = tens;
        self.memory[self.i as usize + 2] = ones;
    }

    /// Stores registers V0 through Vx in memory starting at location I (opcode `Fx55`).
    fn copy_registers_to_memory(&mut self, x: U4){
        for count in 0..(x as usize)+1{
            self.memory[self.i as usize + count] = self.registers[count];
        }
    }

    /// Reads registers V0 through Vx from memory starting at location I (opcode `Fx65`).
    fn copy_memory_into_registers(&mut self, x: U4){
        for count in 0..(x as usize)+1{
            self.registers[count] = self.memory[self.i as usize + count];
        }
    }
}