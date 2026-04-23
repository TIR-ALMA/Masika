fn main() {
    cc::Build::new().file("src/arch/entry.s").compile("entry");
}

