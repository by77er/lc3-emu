mod lc3;
use lc3::LC3;

fn main() {
    let mut lc3 = LC3::new();
    lc3.pc = 0x0200;
    print_registers(&lc3);
}

fn print_registers(lc3: &LC3) {
    println!("-- Registers ---------------");
    println!("r0: {:04x} r1: {:04x} r2: {:04x}", lc3.r0, lc3.r1, lc3.r2);
    println!("r3: {:04x} r4: {:04x} r5: {:04x}", lc3.r3, lc3.r4, lc3.r5);
    println!("r6  (stack): {:04x}", lc3.r6);
    println!("r7    (ret): {:04x}", lc3.r7);
    println!("----------------------------");
}
