use bambam_omf::app::OmfApp;
use clap::Parser;

fn main() {
    env_logger::init();
    let args = OmfApp::parse();
    args.op.run().unwrap()
}
