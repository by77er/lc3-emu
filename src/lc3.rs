#![allow(overflowing_literals, dead_code)]
// for crying out loud

#[derive(Debug, Copy, Clone)]
pub enum LC3IO {
    Halt,
    Display(i16),
    None
}

/// LC-3 (Little Computer 3)
#[derive(Debug)]
pub struct LC3 {
    last_io: LC3IO,
    pub halted: bool, // processor stop and start
    ie: u8, // interrupt enable
    pub pc: i16, // instruction pointer
    pub psr: i16, // process status

    pub saved_usp: i16, // user stack ptr
    pub saved_ssp: i16, // supervisor stack ptr
    
    pub r0: i16, // temp
    pub r1: i16, // temp
    pub r2: i16, // temp
    pub r3: i16, // temp
    pub r4: i16, // .data
    pub r5: i16, // frame pointer
    pub r6: i16, // stack pointer
    pub r7: i16, // return address
    pub memory: LC3Memory
}

/// LC-3 Memory (also manages mmapped IO, protection)
pub struct LC3Memory { 
    pub mem: [i16; 65536],
    keyboard_ready: bool,
    last_char: Option<i16>
    // more stuff for memory mapped io
}

impl std::fmt::Debug for LC3Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "...")
    }
}

// LC3 Memory layout:

// 0x0000
//  Trap Vector Table
// 0x0100
//  Interrupt Vector Table
// 0x0200
//  Operating system and Supervisor Stack
// 0x3000
//  User program and User Stack
// 0xFE00
//  Device register addresses
// 0xFFFF

impl LC3 {
    pub fn new() -> Self {
	Self {
	    last_io: LC3IO::None,
	    halted: true, // starts halted
	    ie: 0b1,
	    pc: 0,
	    psr: 0,

	    saved_usp: 0,
	    saved_ssp: 0,

	    r0: 0,
	    r1: 0,
	    r2: 0,
	    r3: 0,
	    r4: 0,
	    r5: 0,
	    r6: 0,
	    r7: 0,
	    memory: LC3Memory::new() // starts 0'd
	}
    }

    pub fn start(&mut self) {
	self.halted = false;
	self.memory.put(0xFFFE, 0b1);
    }
    
    /// Executes one Fetch Decode Execute cycle
    pub fn clock(&mut self) -> LC3IO {
	if !self.halted {
	    // fetch
	    let instruction = self.memory.get(self.pc as u16);
	    self.pc = self.pc.wrapping_add(1);
	    // decode
	    let code = (instruction as u16 & 0b1111000000000000) >> 12;
	    // execute based on the code
	    match code {
		0b0001 => self.add(instruction),
		0b0101 => self.and(instruction),
		0b0000 => self.br(instruction),
		0b1100 => self.jmp(instruction),
		0b0100 => self.jsr(instruction),
		0b0010 => self.ld(instruction),
		0b1010 => self.ldi(instruction),
		0b0110 => self.ldr(instruction),
		0b1110 => self.lea(instruction),
		0b1001 => self.not(instruction),
		0b1000 => self.rti(instruction), // Causes exception in user mode
		0b0011 => self.st(instruction),
		0b1011 => self.sti(instruction),
		0b0111 => self.stor(instruction), // str name conflict
		0b1111 => self.trap(instruction),
		_ => self.exception(1) // Illegal opcode exception
	    }
	}

	
	// check memory for char
	if self.memory.last_char.is_some() {
	    self.last_io = LC3IO::Display(self.memory.last_char.unwrap());
	    self.memory.last_char = None;
	}
	// check memory for halt
	if self.memory.get(0xFFFE) == 0b0 {
	    self.halted = true;
	    self.last_io = LC3IO::Halt;
	}
	let tmp = self.last_io;
	self.last_io = LC3IO::None;
	tmp
    }

    /// External interrupt
    pub fn interrupt(&mut self, code: u8, priority: u8, data: i16) -> Result<u8, &'static str> {
	// check interrupt enable
	if !(self.ie == 0b1) {
	    return Err("Interrupt Enable is 0");
	}
	// check priority in psr
	let prio = (self.psr >> 8) as u8 & 0b111;
	if prio >= priority {
	    return Err("Currently servicing a higher or equal priority task.");
	}

	// set keyboard input memory
	self.memory.put(0xFE02, data);
	// set keyboard ready
	self.memory.keyboard_ready = true;

	self.saved_usp = self.r6;
	self.r6 = self.saved_ssp;
	self.r6 = self.r6.wrapping_sub(1);
	self.memory.put(self.r6 as u16, self.psr);
	self.r6 = self.r6.wrapping_sub(1);
	self.memory.put(self.r6 as u16, self.pc);
	self.psr &= 0b0_111_1000_1111_1111;
	self.psr |= (priority as i16 & 0b111) << 8;
	self.pc = self.memory.get(0x100 as u16 + code as u16);
	
	Ok(priority)
    }

    /// Internal exception
    fn exception(&mut self, code: u8) {
	self.saved_usp = self.r6;
	self.r6 = self.saved_ssp;
	self.r6 = self.r6.wrapping_sub(1);
	self.memory.put(self.r6 as u16, self.psr);
	self.r6 = self.r6.wrapping_sub(1);
	self.memory.put(self.r6 as u16, self.pc);
	self.psr &= 0b0_111_1111_1111_1111;
	self.pc = self.memory.get(0x100 as u16  + code as u16);
    }

    /// Takes a 3b register code and produces an exclusive ref to the proper register
    fn reg(&mut self, code: i16) -> &mut i16 {
	match code & 0b111 {
	    0 => &mut self.r0,
	    1 => &mut self.r1,
	    2 => &mut self.r2,
	    3 => &mut self.r3,
	    4 => &mut self.r4,
	    5 => &mut self.r5,
	    6 => &mut self.r6,
	    7 => &mut self.r7,
	    _ => unreachable!()
	}
    }

    /// Gets the value of a register based on its 3b code
    fn get_reg(&mut self, code: i16) -> i16 {
	*self.reg(code)
    }

    /// Sets the value of a register based on its 3b code
    fn put_reg(&mut self, code: i16, data: i16) {
	*self.reg(code) = data
    }

    /// Sets the NZP bits of the PSR
    fn codes(&mut self, value: i16) {
	// clear bottom three bits of PSR
	self.psr &= 0b1111_1111_1111_11_000;
	if value == 0 { // set Z
	    self.psr |= 0b010;
	} else if value > 0 { // set P
	    self.psr |= 0b001;
	} else if value < 0 { // set N
	    self.psr |= 0b100;
	}
    }

    /// Implements the LC-3's ADD instruction
    fn add(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
	let sr1 = (instruction >> 6) & 0b111;
	if mux(instruction) { // immediate
	    let imm = sign_extend(instruction & 0b11111, 5);
	    let res = self.get_reg(sr1).wrapping_add(imm);
	    self.codes(res);
	    self.put_reg(dr, res);
	} else { // register
	    let sr2 = instruction & 0b111;
	    let res = self.get_reg(sr1) + self.get_reg(sr2);
	    self.codes(res);
	    self.put_reg(dr, res);
	}
    }

    /// AND instruction
    fn and(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
        let sr1 = (instruction >> 6) & 0b111;
	if mux(instruction) { // immediate
	    let imm = sign_extend(instruction & 0b11111, 5);
	    let res = self.get_reg(sr1) & imm;
	    self.codes(res);
	    self.put_reg(dr, res);
	} else { // register
	    let sr2 = instruction & 0b111;
	    let res = self.get_reg(sr1) & self.get_reg(sr2);
	    self.codes(res);
	    self.put_reg(dr, res);
	}
    }

    /// BR (branch) instruction
    fn br(&mut self, instruction: i16) {
	// PSR condition codes
	let n = (self.psr >> 2) & 0b1;
	let z = (self.psr >> 1) & 0b1;
 	let p = self.psr & 0b1;

	// requested condition codes
	let i_n = (instruction >> 9) & 0b1;
	let i_z = (instruction >> 10) & 0b1;
	let i_p = (instruction >> 11) & 0b1;

	// check if requested set bits match condition codes
	if (i_n == 1 && n == 1) || (i_z == 1 && z == 1) || (i_p == 1 && p == 1) {
	    self.pc = self.pc.wrapping_add(sign_extend(instruction & 0b111_111_111, 9));
	}
    }

    /// JMP / RET
    fn jmp(&mut self, instruction: i16) {
	let dest = self.get_reg((instruction >> 6) & 0b111);
	self.pc = dest;
    }

    /// JSR / JSRR
    fn jsr(&mut self, instruction: i16) {
	let mode = (instruction >> 11) & 0b1;
	let temp = self.pc;
	if mode == 0b1 { // jsr
	    self.pc = self.pc.wrapping_add(sign_extend(instruction & 0b11111111111, 11));
	} else { // jsrr
	    self.pc = self.get_reg((instruction >> 6) & 0b111);
	}
	self.r7 = temp;
    }

    /// LD
    fn ld(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
	let addr = self.pc.wrapping_add(sign_extend(instruction & 0b111_111_111, 9));
	let res = self.memory.get(addr as u16);
	self.codes(res);
	self.put_reg(dr, res);
    }

    /// LDI
    fn ldi(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
	let addr = self.pc.wrapping_add(sign_extend(instruction & 0b111_111_111, 9));
	let addr2 = self.memory.get(addr as u16);
	let res = self.memory.get(addr2 as u16);
	self.codes(res);
	self.put_reg(dr, res);
    }

    /// LDR
    fn ldr(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
	let base_r = self.get_reg((instruction >> 6) & 0b111);
	let offset = sign_extend(instruction & 0b111111, 6);
	let res = self.memory.get((base_r.wrapping_add(offset)) as u16);
	self.codes(res);
	self.put_reg(dr, res);
    }

    /// LEA
    fn lea(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
	let addr = self.pc.wrapping_add(sign_extend(instruction & 0b111_111_111, 9));
	self.codes(addr);
	self.put_reg(dr, addr);
    }

    /// NOT
    fn not(&mut self, instruction: i16) {
	let dr = (instruction >> 9) & 0b111;
	let sr = (instruction >> 6) & 0b111;
	let res = !self.get_reg(sr);
	self.codes(res);
	self.put_reg(dr, res);
    }

    /// RTI - return from interrupt (needs priv == 0)
    fn rti(&mut self, _instruction: i16) {
	let priv_bit = (self.psr >> 15) & 0b1;
	if priv_bit == 0b0 { // ok
	    // pop pc from supervisor stack
	    self.pc = self.memory.get(self.r6 as u16);
	    self.r6 = self.r6.wrapping_add(1);
	    // pop psr from supervisor stack
	    self.psr = self.memory.get(self.r6 as u16);
	    self.r6 = self.r6.wrapping_add(1);
	    // restore user stack ptr
	    self.saved_ssp = self.r6;
	    self.r6 = self.saved_usp;
	} else { // not ok, priv exception
	    self.exception(0);
	}
    }

    /// ST
    fn st(&mut self, instruction: i16) {
	let sr = self.get_reg((instruction >> 9) & 0b111);
	let addr = self.pc.wrapping_add(sign_extend(instruction & 0b111_111_111, 9));
	self.memory.put(addr as u16, sr);
    }

    /// STI
    fn sti(&mut self, instruction: i16) {
	let sr = self.get_reg((instruction >> 9) & 0b111);
	let addr = self.pc.wrapping_add(sign_extend(instruction & 0b111_111_111, 9));
	let addr2 = self.memory.get(addr as u16);
	self.memory.put(addr2 as u16, sr);
    }

    /// STR
    fn stor(&mut self, instruction: i16) {
	let sr = self.get_reg((instruction >> 9) & 0b111);
	let base_r = self.get_reg((instruction >> 6) & 0b111);
	let addr = base_r.wrapping_add(sign_extend(instruction & 0b111111, 6));
	self.memory.put(addr as u16, sr);
    }

    /// TRAP
    fn trap(&mut self, instruction: i16) {
	self.r7 = self.pc;
	let vector_index = instruction as u16 & 0b11111111;
	self.pc = self.memory.get(vector_index);
    }
    
}

/// Mode switch on ADD and ADD instructions
fn mux(instruction: i16) -> bool {
    instruction & 0b100000 == 0b100000
}

/// 2's complement sign-extension
fn sign_extend(value: i16, length: usize) -> i16 {
    let mut ctr = length;
    let mut out = value;
    // bit determines if negative or positive
    let bit = (value >> (length - 1)) & 0b1;
    while ctr < 16 {
	out |= bit << ctr;
	ctr += 1;
    }
    out
}

impl LC3Memory {
    pub fn new() -> Self {
	Self {
	    mem: [0; 65536],
	    keyboard_ready: false,
	    last_char: None
	}
    }
    pub fn get(&mut self, index: u16) -> i16 {
	if index == 0xFE04 { // Display is always ready (?)
	    return 0b1;
	} else if index == 0xFE00 { // keyboard ready
	    if self.keyboard_ready {
		return 0b1;
	    } else {
		return 0b0;
	    }
	} else if index == 0xFE02 {
	    self.keyboard_ready = false;
	}
	return self.mem[index as usize];
    }
    pub fn put(&mut self, index: u16, value: i16) {
	// println!("put {:04x} @ {:04x}", value, index);
	if index == 0xFE06 { // write here so cpu can check
	    self.last_char = Some(value)
	}
	self.mem[index as usize % 65536] = value;
    }
}


#[cfg(test)]
mod tests {
    use super::LC3;
    use super::{mux, sign_extend};
    
    #[test]
    fn creation() {
        let _lc3 = LC3::new();
    }

    #[test]
    fn mux_test() {
	assert_eq!(false, mux(0b0101000000000000));
	assert_eq!(true, mux(0b0001000000100000));
    }

    #[test]
    fn sign_extension() {
	let goal = 0b1111111111110100i16;
	let strt = 0b0000000000110100i16;
	assert_eq!(goal, sign_extend(strt, 6));
	
	let goal = 0b0000000000000100i16;
	let strt = 0b0000000000000100i16;
	assert_eq!(goal, sign_extend(strt, 6));
    }

    #[test]
    fn add_test() {
	// immediate
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0001_001_010_1_01111); // ADD R1, R2, #1
	lc3.pc = 0x3000;
	lc3.r2 = 100;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r1, 100 + sign_extend(0b01111, 5));

	// register
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b001_001_010_0_00_011); // ADD R1, R2, R3
	lc3.pc = 0x3000;
	lc3.r2 = 100;
	lc3.r3 = -50;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r1, 50);
	assert_eq!(lc3.psr & 0b111, 0b001);
    }

    #[test]
    fn and_test() {
	// immediate
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0101_001_001_1_00000); // AND R1, R1, #0 ; zero R1
	lc3.pc = 0x3000;
	lc3.r1 = -1283;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r1, 0);
	assert_eq!(lc3.psr & 0b111, 0b010);
	
	// register
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0101_001_001_0_00_001); // AND R1, R1, R1 ; do nothing
	lc3.pc = 0x3000;
	lc3.r1 = -1283;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r1, -1283);
	assert_eq!(lc3.psr & 0b111, 0b100);
    }

    #[test]
    fn branch_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0101_001_001_1_00000); // AND R1, R1, #0 ; zero R1                      
	lc3.memory.put(0x3001, 0b0000_110_000000101); // BRnz #5
	lc3.pc = 0x3000;                                                                                 
        lc3.r1 = -1283;                                                                                  
        lc3.halted = false;                                                                              
        lc3.clock();
	// Z should now be set
	lc3.clock();
	assert_eq!(lc3.pc, 0x3007);
    }

    #[test]
    fn jmp_test() {
	let mut lc3 = LC3::new();
        lc3.memory.put(0x3000, 0b1100_000_010_000000); // JMP R2
	lc3.pc = 0x3000;
	lc3.r2 = 0x3500;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.pc, 0x3500);
    }

    #[test]
    fn jsr_test() {
	// JSR
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0100_1_11111111110); // JSR #-2 ; jump back by 2
	lc3.pc = 0x3000;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.pc, 0x2FFF);
	assert_eq!(lc3.r7, 0x3001); // check for saved pc

	// JSRR
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0100_0_00_010_000000); // JSRR R2
	lc3.pc = 0x3000;
	lc3.r2 = 0xB33F;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.pc, 0xB33F);
	assert_eq!(lc3.r7, 0x3001);
    }

    #[test]
    fn ld_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3002, 0xB773);
	lc3.memory.put(0x3000, 0b0010_010_000000001);
	lc3.pc = 0x3000;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r2, 0xB773);
    }

    #[test]
    fn ldi_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3002, 0xF33D);
	lc3.memory.put(0xF33D, 0xB33F);
	lc3.memory.put(0x3000, 0b1010_010_000000001);
	lc3.pc = 0x3000;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r2, 0xB33F);
    }

    #[test]
    fn ldr_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0110_010_001_000001);
	lc3.memory.put(0x5001, 0x5372);
	lc3.pc = 0x3000;
	lc3.r1 = 0x5000;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r2, 0x5372);
    }

    #[test]
    fn lea_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b1110_010_000000010);
	lc3.pc = 0x3000;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r2, 0x3003);
    }

    #[test]
    fn not_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b1001_010_001_1_11111);
	lc3.pc = 0x3000;
	lc3.r1 = 0b1010101010101010;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.r2, 0b0101010101010101);
    }

    #[test]
    fn st_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0011_010_000000001);
	lc3.pc = 0x3000;
	lc3.r2 = 0x2534;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.memory.get(0x3002), 0x2534);
    }

    #[test]
    fn sti_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b1011_010_000000001);
	lc3.memory.put(0x3002, 0x5000);
	lc3.pc = 0x3000;
	lc3.r2 = 0xB33F;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.memory.get(0x5000), 0xB33F)
    }

    #[test]
    fn stor_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b0111_010_001_000001);
	lc3.pc = 0x3000;
	lc3.r2 = 0xFEED;
	lc3.r1 = 0x0001;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.memory.get(0x0002), 0xFEED);
    }

    #[test]
    fn trap_test() {
	let mut lc3 = LC3::new();
	lc3.memory.put(0x3000, 0b1111_0000_00000001);
	lc3.memory.put(0x0001, 0x1337);
	lc3.pc = 0x3000;
	lc3.halted = false;
	lc3.clock();
	assert_eq!(lc3.pc, 0x1337);
    }

    #[test]
    fn exception_test() {
	let mut lc3 = LC3::new();
	lc3.psr |= 0b1 << 15; // user priv
	lc3.memory.put(0x0100, 0x0200); // IVT pointer
	lc3.memory.put(0x0200, 0b1000_0000_0000_0000); // proper rti in supervisor mode
	lc3.memory.put(0x3000, 0b1000_0000_0000_0000); // illegal rti, triggers exception
	lc3.memory.put(0x3001, 0b001_000_000_1_00001); // add 1 to r0 to show it continued
	lc3.pc = 0x3000;
	lc3.saved_ssp = 0x3000; // base of supervisor stack
	lc3.halted = false;
	lc3.clock();
	println!("before: {:#?}", lc3);
	lc3.clock();
	lc3.clock();
	println!("after: {:#?}", lc3);
	assert_eq!(lc3.r0, 1);
	assert_eq!(lc3.memory.get(0x3000 - 2), 0x3001);
    }

    #[test]
    fn interrupt_test() {
	let mut lc3 = LC3::new();
	lc3.psr |= 0b1 << 15; // user mode
	lc3.pc = 0x3000;
	lc3.memory.put(0x100 + 0x80, 0x1200); // interrupt handler
	lc3.memory.put(0x1200, 0b1000_0000_0000_0000); // rti
	lc3.saved_ssp = 0x3000;
	lc3.halted = false;
	lc3.interrupt(0x80, 1, 'A' as i16).expect("Failed to interrupt");
	println!("{:#?}", lc3);
	lc3.clock();
	println!("{:#?}", lc3);
	// panic!();
	assert_eq!(lc3.memory.get(0xFE02), 'A' as i16);
    }
}
