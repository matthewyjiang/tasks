fn main() {
    uniffi::generate_scaffolding("uniffi/core.udl").expect("generate UniFFI scaffolding");
}
