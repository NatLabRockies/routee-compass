use bambam_omf::{app::OmfApp, collection::OvertureMapsCollectionError};
use clap::Parser;

fn main() -> Result<(), OvertureMapsCollectionError> {
    env_logger::init();
    let args = OmfApp::parse();
    args.op.run()
}
