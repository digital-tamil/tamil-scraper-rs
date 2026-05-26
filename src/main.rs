mod songs;
struct TamilWebScrapper {}

impl TamilWebScrapper {
    fn thiruppugazh(output_path: &str) {
        songs::thiruppugazh::thiruppugazh(output_path);
    }

    fn thirumurai(output_path: &str) {
        songs::thirumurai::thirumurai(output_path);
    }
}
fn main() {
    TamilWebScrapper::thirumurai("data/thirumurai.json");
    TamilWebScrapper::thiruppugazh("data/thiruppugazh.json");
}
