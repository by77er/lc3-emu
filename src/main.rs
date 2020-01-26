#![allow(overflowing_literals)]

mod lc3;
use lc3::{LC3, LC3IO};

// use std::io;

fn main() {
    let mut lc3 = LC3::new();
    prepare_supervisor(&mut lc3);

    prepare_user_program(&mut lc3);
    
    lc3.psr = 0b1 << 15;    // user-mode privileges
    lc3.pc = 0x3000;        // Set program counter to start of user program space
    lc3.saved_ssp = 0x3000; // Supervisor stack starts right on top of user program space
    lc3.r6 = 0xFE00;        // Ready user program stack pointer
    print_registers(&mut lc3);

    println!(); // spacing
    
    lc3.start();
    
    let mut done = false;
    while !done {
	// print_registers(&mut lc3);

	// std::io::stdin().read_line(&mut String::new());
	
	let r = lc3.clock();
	match r {
	    LC3IO::None => (),
	    LC3IO::Display(c) => print!("{}", (c as u8) as char),
	    LC3IO::Halt => {
		done = true;
		println!("\n -- Processor halted at 0x{:04x} -- ", lc3.pc);
	    }
	}
    }
}

fn prepare_user_program(lc3: &mut LC3) {
    // load r0 with char
    lc3.memory.put(0x3000, 0b1110_000_000000010);   // LEA R0, [PC + 2]
    lc3.memory.put(0x3001, 0b1111_0000_00100010);   // TRAP 0x22 (PUTS)
    lc3.memory.put(0x3002, 0b1111_0000_00100101);   // TRAP 0x25 (HALT)
    lc3.memory.put(0x3003, 0x48);                   
    lc3.memory.put(0x3004, 0x45);
    lc3.memory.put(0x3005, 0x4C);
    lc3.memory.put(0x3006, 0x4C);
    lc3.memory.put(0x3007, 0x4F);
    lc3.memory.put(0x3008, 0x20);
    lc3.memory.put(0x3009, 0x57);
    lc3.memory.put(0x300a, 0x4F);
    lc3.memory.put(0x300b, 0x52);
    lc3.memory.put(0x300c, 0x4c);
    lc3.memory.put(0x300d, 0x44);
    lc3.memory.put(0x300e, 0x0A);
    lc3.memory.put(0x300f, 0x00);
}

fn prepare_supervisor(lc3: &mut LC3) {
    // trap vector table
    lc3.memory.put(0x0020, 0x0200); // getc  (read a single character from the keyboard to r0)
    lc3.memory.put(0x0021, 0x0220); // out   (write r0 to console)
    lc3.memory.put(0x0022, 0x0240); // puts  (write string pointed to by r0 until 0x0000)
    lc3.memory.put(0x0023, 0x0260); // in    (getc with echo)
    lc3.memory.put(0x0024, 0x0280); // putsp (puts but packed 2 chars per memory location)
    lc3.memory.put(0x0025, 0x02A0); // halt  (stop the LC3)
    // interrupt vector table
    lc3.memory.put(0x0100, 0x02C0); // priv
    lc3.memory.put(0x0101, 0x02C0); // illegal
    lc3.memory.put(0x0180, 0x02E0); // keystroke
    
    // trap code

    //  GETC FE00 Status FE02 Data
    lc3.memory.put(0x0200, 0b1010_000_000000010); // LDI R0, [PC + 2] ; load *0x203 -> *FE00 into r0
    lc3.memory.put(0x0201, 0b0000_010_000000001); // BRz  PC - 2      ; r0 == 0, nothing, retry
    lc3.memory.put(0x0202, 0b0000_000_000000001); // BR   PC + 1      ; continue
    lc3.memory.put(0x0203, 0xFE00);               // db 0xFE00        ; Keyboard Status
    lc3.memory.put(0x0204, 0b1010_000_000000001); // LDI R0, [PC + 1] ; load *0x206 -> *FE02 into r0
    lc3.memory.put(0x0205, 0b1100_000_111_000000);// RET
    lc3.memory.put(0x0206, 0xFE02);               // db 0xFE02

    //  OUT FE06 Data
    lc3.memory.put(0x0220, 0b1011_000_000000001); // STI R0, [PC + 1] ; put R0 into display reg
    lc3.memory.put(0x0221, 0b1100_000_111_000000);// RET
    lc3.memory.put(0x0222, 0xFE06);

    //  PUTS
    lc3.memory.put(0x0240, 0b0001_001_111_1_00000); // ADD R1, R7, #0    ; save RET register
    lc3.memory.put(0x0241, 0b0001_010_000_1_00000); // ADD R2, R0, #0    ; move r0 to r2
    lc3.memory.put(0x0242, 0b0110_000_010_000000); // LDR R0, [R2 + #0]  ; load character to r0
    lc3.memory.put(0x0243, 0b0000_010_000000011);  // BRz PC + 3         ; if zero, go to return
    lc3.memory.put(0x0244, 0b1111_0000_00100001);  // TRAP 0x21 (OUT)    ; print character
    lc3.memory.put(0x0245, 0b0001_010_010_1_00001);// ADD R2, R2, #1     ; increment string ptr
    lc3.memory.put(0x0246, 0b0000_111_111111011);  // BR PC - 5          ; go 5 back
    lc3.memory.put(0x0247, 0b0001_111_001_1_00000); // ADD R7, R1, #0    ; return address back to r7
    lc3.memory.put(0x0248, 0b1100_000_111_000000); // RET
	
    
    // TODO ...
    
    //  HALT FFFE
    lc3.memory.put(0x02A0, 0b0101_000_000_1_00000);// zero r0
    lc3.memory.put(0x02A1, 0b1011_000_000000001);  // STI R0, [PC + 1] ; put R0 into display reg
    lc3.memory.put(0x02A2, 0b1100_000_111_000000); // RET
    lc3.memory.put(0x02A3, 0xFFFE);
    
    // interrupt code
    
} 

fn print_registers(lc3: &mut LC3) {
    println!("-- Registers -----------------");
    println!("pc: {:04x} -> {:016b}", lc3.pc, lc3.memory.get(lc3.pc as u16));
    println!("psr: {:016b}", lc3.psr);
    println!("r0: {:04x}  r1: {:04x} r2: {:04x}", lc3.r0, lc3.r1, lc3.r2);
    println!("r3: {:04x}  r4: {:04x} r5: {:04x}", lc3.r3, lc3.r4, lc3.r5);
    println!("r6  (stack) : {:04x}", lc3.r6);
    println!("r7  (ret)   : {:04x}", lc3.r7);
    println!("------------------------------");
}
